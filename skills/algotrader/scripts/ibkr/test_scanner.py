# /// script
# requires-python = ">=3.12"
# dependencies = ["ib_async"]
# ///
"""Test IBKR market scanner API.

Connects via ib_async and uses reqScannerData to run market scanners.
Scanners work WITHOUT a market data subscription, but only return results
during US market hours (9:30 AM – 4:00 PM ET, Mon–Fri).

Prerequisites:
    - TWS Classic or IB Gateway running with API enabled
    - TWS: Edit > Global Configuration > API > Settings > Enable ActiveX and Socket Clients
    - IB Gateway: API enabled by default
    - IBKR Desktop does NOT support TWS API — use TWS Classic or IB Gateway

Ports:
    TWS live=7496, TWS paper=7497, Gateway live=4001, Gateway paper=4002

Scanner gotchas:
    - Use --location STK.US (not STK.US.MAJOR — returns empty)
    - Use --min-cap 0 (marketCapAbove filter needs fundamentals subscription)
    - Max 50 results per scan, max 10 concurrent scanner subscriptions
    - Results refresh every ~30 seconds with reqScannerSubscription (this script
      uses one-shot reqScannerData instead)

Examples:
    uv run scripts/ibkr/test_scanner.py                          # default: HIGH_REL_VOLUME
    uv run scripts/ibkr/test_scanner.py --scan MOST_ACTIVE
    uv run scripts/ibkr/test_scanner.py --scan TOP_PERC_GAIN
    uv run scripts/ibkr/test_scanner.py --scan TOP_PERC_LOSE
    uv run scripts/ibkr/test_scanner.py --list-scans             # dump all 75+ scan codes
    uv run scripts/ibkr/test_scanner.py --location STK.US --min-cap 0

Troubleshooting:
    ConnectionRefusedError  -> TWS/Gateway not running, or API not enabled, or wrong port
    No results returned     -> Outside market hours, or wrong location code
    Empty with --min-cap >0 -> Fundamentals subscription needed for cap filter; use --min-cap 0
"""

from __future__ import annotations

import argparse
import xml.etree.ElementTree as ET

from ib_async import IB, ScannerSubscription


def list_scan_codes(ib: IB) -> None:
    """Pull available scan codes from TWS and print them."""
    xml_str = ib.reqScannerParameters()
    root = ET.fromstring(xml_str)

    print("=== Available Scan Codes ===\n")
    for elem in root.iter("ScanType"):
        code = elem.findtext("scanCode", "")
        name = elem.findtext("displayName", "")
        if code:
            print(f"  {code:40s} {name}")

    print(f"\n=== Available Location Codes ===\n")
    for elem in root.iter("LocationType"):
        code = elem.findtext("locationCode", "")
        name = elem.findtext("displayName", "")
        if code and "STK" in code:
            print(f"  {code:40s} {name}")


def run_scan(ib: IB, scan_code: str, location: str, above_price: float, min_cap: float) -> None:
    """Run a scanner and print results."""
    scan = ScannerSubscription(
        instrument="STK",
        locationCode=location,
        scanCode=scan_code,
        abovePrice=above_price,
        marketCapAbove=min_cap,
    )

    print(f"Running scan: {scan_code} on {location} (price>${above_price}, cap>${min_cap/1e9:.0f}B)...\n")
    results = ib.reqScannerData(scan)

    if not results:
        print("No results returned.")
        return

    print(f"{'#':>3}  {'Symbol':10s} {'Exchange':10s} {'Type':6s} {'ConId':>10s}")
    print("-" * 50)
    for i, item in enumerate(results):
        cd = item.contractDetails
        c = cd.contract
        print(f"{item.rank:3d}  {c.symbol:10s} {c.primaryExchange:10s} {c.secType:6s} {c.conId:>10d}")

    print(f"\n{len(results)} results")


def main():
    p = argparse.ArgumentParser(
        description="Test IBKR market scanners",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
examples:
  uv run scripts/ibkr/test_scanner.py                          # HIGH_REL_VOLUME (default)
  uv run scripts/ibkr/test_scanner.py --scan MOST_ACTIVE
  uv run scripts/ibkr/test_scanner.py --scan TOP_PERC_GAIN --location STK.US --min-cap 0
  uv run scripts/ibkr/test_scanner.py --list-scans

gotchas:
  - Use --location STK.US (not STK.US.MAJOR — returns empty)
  - Use --min-cap 0 (marketCapAbove filter needs fundamentals subscription)
  - Scanners only return results during US market hours
""",
    )
    p.add_argument("--host", default="127.0.0.1")
    p.add_argument("--port", type=int, default=7496)
    p.add_argument("--scan", default="HIGH_REL_VOLUME", help="Scan code to run")
    p.add_argument("--location", default="STK.US.MAJOR", help="Location code")
    p.add_argument("--min-price", type=float, default=5.0)
    p.add_argument("--min-cap", type=float, default=1e9, help="Min market cap in USD")
    p.add_argument("--list-scans", action="store_true", help="List available scan codes and exit")
    args = p.parse_args()

    ib = IB()
    ib.connect(args.host, args.port, clientId=30)

    if args.list_scans:
        list_scan_codes(ib)
    else:
        run_scan(ib, args.scan, args.location, args.min_price, args.min_cap)

    ib.disconnect()


if __name__ == "__main__":
    main()
