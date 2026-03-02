# /// script
# requires-python = ">=3.12"
# dependencies = ["ib_async", "fastapi", "uvicorn"]
# ///
"""IBKR REST API daemon — persistent TWS connection with HTTP endpoints.

Maintains a single ib_async connection to TWS/IB Gateway and exposes
a FastAPI REST surface for querying positions, portfolio, historical
bars, and market scanners.

Prerequisites:
    - TWS Classic or IB Gateway running with API enabled
    - TWS: Edit > Global Configuration > API > Settings > Enable ActiveX and Socket Clients
    - IB Gateway: API enabled by default
    - IBKR Desktop does NOT support TWS API — use TWS Classic or IB Gateway

Ports:
    TWS live=7496, TWS paper=7497, Gateway live=4001, Gateway paper=4002

Endpoints:
    GET  /health          Connection status and account info
    GET  /positions       Open positions (cached)
    GET  /portfolio       Portfolio with market values and PnL (cached)
    GET  /orders          Open orders with fill status (cached)
    GET  /bars/{symbol}   Historical OHLCV bars (async TWS request)
    POST /scanner         Run a market scanner (async TWS request)
    GET  /scanner/codes   List available scan codes and locations

Examples:
    uv run scripts/ibkr/daemon.py                    # TWS live, HTTP on :8000
    uv run scripts/ibkr/daemon.py --port 4002        # IB Gateway paper
    uv run scripts/ibkr/daemon.py --http-port 9000   # custom HTTP port

    curl localhost:8000/health
    curl localhost:8000/positions
    curl "localhost:8000/bars/SPY?duration=90+D&bar_size=1+hour"
    curl -X POST localhost:8000/scanner -H 'Content-Type: application/json' \\
      -d '{"scan_code":"MOST_ACTIVE","location":"STK.US"}'

    # Interactive API docs
    open http://localhost:8000/docs

Troubleshooting:
    ConnectionRefusedError  -> TWS/Gateway not running, or API not enabled, or wrong port
    503 on all endpoints    -> TWS disconnected; restart daemon or TWS
    429 on /bars            -> Rate limit (60 requests/10min); wait for Retry-After header
    Scanner returns []      -> Outside market hours, or wrong location (use STK.US not STK.US.MAJOR)
"""

from __future__ import annotations

import argparse
import time
import xml.etree.ElementTree as ET
from collections import deque
from contextlib import asynccontextmanager

import uvicorn
from fastapi import Depends, FastAPI, HTTPException, Response
from ib_async import IB, ScannerSubscription, Stock
from pydantic import BaseModel

# ---------------------------------------------------------------------------
# Globals (set from CLI args before uvicorn starts)
# ---------------------------------------------------------------------------

ib = IB()
_args: argparse.Namespace | None = None

# Rate limit tracking for historical data requests
_hist_timestamps: deque[float] = deque()
HIST_LIMIT = 60
HIST_WINDOW = 600  # 10 minutes


# ---------------------------------------------------------------------------
# Lifespan + dependencies
# ---------------------------------------------------------------------------


@asynccontextmanager
async def lifespan(_app: FastAPI):
    assert _args is not None
    await ib.connectAsync(_args.host, _args.port, clientId=_args.client_id)
    try:
        yield
    finally:
        ib.disconnect()


app = FastAPI(title="IBKR REST API", lifespan=lifespan)


async def require_connection():
    if not ib.isConnected():
        raise HTTPException(status_code=503, detail="Not connected to TWS/Gateway")


# ---------------------------------------------------------------------------
# Rate limit helpers
# ---------------------------------------------------------------------------


def check_hist_rate_limit():
    now = time.monotonic()
    while _hist_timestamps and _hist_timestamps[0] < now - HIST_WINDOW:
        _hist_timestamps.popleft()
    if len(_hist_timestamps) >= HIST_LIMIT:
        wait = _hist_timestamps[0] + HIST_WINDOW - now
        raise HTTPException(
            status_code=429,
            detail=f"Historical data rate limit ({HIST_LIMIT}/{HIST_WINDOW // 60}min). Retry in {wait:.0f}s.",
            headers={"Retry-After": str(int(wait) + 1)},
        )


def hist_remaining() -> int:
    now = time.monotonic()
    while _hist_timestamps and _hist_timestamps[0] < now - HIST_WINDOW:
        _hist_timestamps.popleft()
    return HIST_LIMIT - len(_hist_timestamps)


# ---------------------------------------------------------------------------
# Endpoints
# ---------------------------------------------------------------------------


@app.get("/health")
async def health():
    return {
        "connected": ib.isConnected(),
        "host": ib.client.host if ib.client else None,
        "port": ib.client.port if ib.client else None,
        "client_id": ib.client.clientId if ib.client else None,
        "accounts": ib.managedAccounts(),
    }


@app.get("/positions", dependencies=[Depends(require_connection)])
async def positions():
    return [
        {
            "account": p.account,
            "symbol": p.contract.symbol,
            "sec_type": p.contract.secType,
            "exchange": p.contract.exchange,
            "con_id": p.contract.conId,
            "position": float(p.position),
            "avg_cost": float(p.avgCost),
        }
        for p in ib.positions()
    ]


@app.get("/portfolio", dependencies=[Depends(require_connection)])
async def portfolio():
    return [
        {
            "account": item.account,
            "symbol": item.contract.symbol,
            "sec_type": item.contract.secType,
            "con_id": item.contract.conId,
            "position": float(item.position),
            "market_price": float(item.marketPrice),
            "market_value": float(item.marketValue),
            "avg_cost": float(item.averageCost),
            "unrealized_pnl": float(item.unrealizedPNL),
            "realized_pnl": float(item.realizedPNL),
        }
        for item in ib.portfolio()
    ]


@app.get("/orders", dependencies=[Depends(require_connection)])
async def open_orders():
    return [
        {
            "order_id": t.order.orderId,
            "symbol": t.contract.symbol,
            "sec_type": t.contract.secType,
            "action": t.order.action,
            "order_type": t.order.orderType,
            "total_qty": float(t.order.totalQuantity),
            "lmt_price": float(t.order.lmtPrice) if t.order.lmtPrice else None,
            "status": t.orderStatus.status,
            "filled": float(t.orderStatus.filled),
            "remaining": float(t.orderStatus.remaining),
            "avg_fill_price": float(t.orderStatus.avgFillPrice),
        }
        for t in ib.openTrades()
    ]


@app.get("/bars/{symbol}", dependencies=[Depends(require_connection)])
async def bars(
    symbol: str,
    response: Response,
    duration: str = "1 Y",
    bar_size: str = "1 day",
    what: str = "TRADES",
    rth: bool = True,
    exchange: str = "SMART",
    currency: str = "USD",
):
    check_hist_rate_limit()

    contract = Stock(symbol.upper(), exchange, currency)
    try:
        result = await ib.reqHistoricalDataAsync(
            contract,
            endDateTime="",
            durationStr=duration,
            barSizeSetting=bar_size,
            whatToShow=what,
            useRTH=rth,
            keepUpToDate=False,
        )
    except Exception as e:
        raise HTTPException(status_code=502, detail=str(e))

    if not result:
        raise HTTPException(status_code=404, detail=f"No data for {symbol}")

    _hist_timestamps.append(time.monotonic())
    response.headers["X-RateLimit-Remaining"] = str(hist_remaining())

    return [
        {
            "date": str(bar.date),
            "open": float(bar.open),
            "high": float(bar.high),
            "low": float(bar.low),
            "close": float(bar.close),
            "volume": int(bar.volume),
            "average": float(bar.average),
            "bar_count": int(bar.barCount),
        }
        for bar in result
    ]


class ScannerRequest(BaseModel):
    scan_code: str = "MOST_ACTIVE"
    location: str = "STK.US"
    instrument: str = "STK"
    above_price: float = 5.0
    market_cap_above: float = 0


@app.post("/scanner", dependencies=[Depends(require_connection)])
async def scanner(req: ScannerRequest):
    scan = ScannerSubscription(
        instrument=req.instrument,
        locationCode=req.location,
        scanCode=req.scan_code,
        abovePrice=req.above_price,
        marketCapAbove=req.market_cap_above,
    )
    try:
        results = await ib.reqScannerDataAsync(scan)
    except Exception as e:
        raise HTTPException(status_code=502, detail=str(e))

    if not results:
        return []

    return [
        {
            "rank": item.rank,
            "symbol": item.contractDetails.contract.symbol,
            "sec_type": item.contractDetails.contract.secType,
            "exchange": item.contractDetails.contract.primaryExchange,
            "con_id": item.contractDetails.contract.conId,
        }
        for item in results
    ]


@app.get("/scanner/codes", dependencies=[Depends(require_connection)])
async def scanner_codes():
    try:
        xml_str = await ib.reqScannerParametersAsync()
    except Exception as e:
        raise HTTPException(status_code=502, detail=str(e))

    root = ET.fromstring(xml_str)

    scan_types = []
    for elem in root.iter("ScanType"):
        code = elem.findtext("scanCode", "")
        name = elem.findtext("displayName", "")
        if code:
            scan_types.append({"code": code, "name": name})

    locations = []
    for elem in root.iter("LocationType"):
        code = elem.findtext("locationCode", "")
        name = elem.findtext("displayName", "")
        if code:
            locations.append({"code": code, "name": name})

    return {"scan_types": scan_types, "locations": locations}


# ---------------------------------------------------------------------------
# CLI + main
# ---------------------------------------------------------------------------


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="IBKR REST API daemon",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
examples:
  uv run scripts/ibkr/daemon.py                    # TWS live, HTTP on :8000
  uv run scripts/ibkr/daemon.py --port 4002        # IB Gateway paper
  uv run scripts/ibkr/daemon.py --http-port 9000   # custom HTTP port

endpoints:
  GET  /health          connection status
  GET  /positions       open positions
  GET  /portfolio       portfolio with PnL
  GET  /orders          open orders
  GET  /bars/{symbol}   historical bars (?duration=1+Y&bar_size=1+day)
  POST /scanner         run market scanner (JSON body)
  GET  /scanner/codes   list scan codes
  GET  /docs            interactive API docs (Swagger UI)
""",
    )
    p.add_argument("--host", default="127.0.0.1", help="TWS/Gateway host (default: 127.0.0.1)")
    p.add_argument("--port", type=int, default=7496, help="7496=TWS live, 7497=paper, 4001/4002=Gateway")
    p.add_argument("--client-id", type=int, default=10, help="TWS client ID (default: 10)")
    p.add_argument("--bind", default="127.0.0.1", help="HTTP bind address (default: 127.0.0.1)")
    p.add_argument("--http-port", type=int, default=8000, help="HTTP port (default: 8000)")
    return p.parse_args()


def main():
    global _args
    _args = parse_args()

    print(f"Starting IBKR REST API daemon")
    print(f"  TWS: {_args.host}:{_args.port} (clientId={_args.client_id})")
    print(f"  HTTP: http://{_args.bind}:{_args.http_port}")
    print(f"  Docs: http://{_args.bind}:{_args.http_port}/docs\n")

    uvicorn.run(app, host=_args.bind, port=_args.http_port, log_level="info")


if __name__ == "__main__":
    main()
