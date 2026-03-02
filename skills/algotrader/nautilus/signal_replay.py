"""Replay pre-computed entry signals from the Rust engine in NautilusTrader.

Reads a CSV of (date, ticker, direction, stop_price) produced by the Rust
engine's --dump-trades flag.  NautilusTrader computes its own fills and
position sizes so the execution model is the sole variable.

Exit rules match signal_breakout.rs (all managed manually, no stop orders):
  1. StopLoss: close <= stop_price
  2. ProfitTarget: close entire position after 5 bars if profitable
  3. SmaCross: close < SMA10

Usage:
    python -m nautilus.signal_replay \
      --data-dir data-crypto \
      --trades /tmp/signal_trades.csv \
      --init-cash 100000
"""

from __future__ import annotations

import argparse
import csv
import sys
import time
from collections import defaultdict
from decimal import Decimal
from pathlib import Path

import pandas as pd

from nautilus_trader.adapters.binance import BINANCE_VENUE
from nautilus_trader.backtest.engine import BacktestEngine
from nautilus_trader.config import BacktestEngineConfig, LoggingConfig, StrategyConfig
from nautilus_trader.indicators import SimpleMovingAverage
from nautilus_trader.model.currencies import USDT
from nautilus_trader.model.data import Bar, BarType
from nautilus_trader.model.enums import AccountType, OmsType, OrderSide, TimeInForce
from nautilus_trader.model.events import PositionClosed
from nautilus_trader.model.identifiers import InstrumentId, TraderId
from nautilus_trader.model.objects import Money
from nautilus_trader.persistence.wranglers import BarDataWrangler
from nautilus_trader.trading.strategy import Strategy

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from scripts.nautilus_backtest import ensure_currency, load_ticker_csv, make_instrument


# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------


class SignalReplayConfig(StrategyConfig, frozen=True):
    """Parameters for signal replay cross-validation."""

    instrument_ids: list[str]
    bar_spec: str = "1-DAY-LAST"
    trades_csv: str = ""

    # Risk management (match Rust engine defaults)
    # Use fixed init_cash for sizing (Rust engine doesn't deplete equity)
    init_cash: float = 100_000.0
    risk_pct: float = 0.005
    max_pos_pct: float = 0.20

    # Exit rules (match signal_breakout.rs)
    profit_target_bars: int = 5   # ProfitTarget min_bars
    trail_period: int = 10        # SMA period for SmaCross exit


# ---------------------------------------------------------------------------
# Strategy
# ---------------------------------------------------------------------------


class SignalReplayStrategy(Strategy):
    """Replay Rust engine entry signals with NautilusTrader execution.

    Stops are managed manually (check close <= stop each bar) instead of
    stop-market orders, because the Rust engine's stop prices may be above
    the entry bar's open when the market gaps down overnight.
    """

    def __init__(self, config: SignalReplayConfig) -> None:
        super().__init__(config)

        # Load signals: (date_str, ticker) -> list of stop_prices
        # Multiple trades per ticker per day are possible (rare but valid)
        self._signals: dict[tuple[str, str], list[float]] = defaultdict(list)
        if config.trades_csv:
            with open(config.trades_csv) as f:
                for row in csv.DictReader(f):
                    key = (row["date"], row["ticker"])
                    self._signals[key].append(float(row["stop_price"]))

        self.bar_types: dict[InstrumentId, BarType] = {}
        self.instruments: dict[InstrumentId, object] = {}
        self._ticker_map: dict[InstrumentId, str] = {}

        # SMA10 trailing exit indicators
        self.sma10: dict[InstrumentId, SimpleMovingAverage] = {}

        # Position tracking (manual stop management)
        self.entry_bar_count: dict[InstrumentId, int] = {}
        self.entry_price: dict[InstrumentId, float] = {}
        self.stop_price: dict[InstrumentId, float] = {}

        # Diagnostics
        self._diag_bars = 0
        self._diag_in_pos_skip = 0
        self._diag_no_signal = 0
        self._diag_stop_above = 0
        self._diag_zero_dist = 0
        self._diag_no_equity = 0
        self._diag_zero_qty = 0
        self._diag_entries = 0
        self._diag_exits_stop = 0
        self._diag_exits_profit = 0
        self._diag_exits_sma = 0
        self._diag_first_bar_dates: list[str] = []

    def on_start(self) -> None:
        for iid_str in self.config.instrument_ids:
            iid = InstrumentId.from_str(iid_str)
            instrument = self.cache.instrument(iid)
            if instrument is None:
                continue

            self.instruments[iid] = instrument
            bt = BarType.from_str(f"{iid_str}-{self.config.bar_spec}-EXTERNAL")
            self.bar_types[iid] = bt
            self._ticker_map[iid] = iid.symbol.value

            # Register SMA10 for trailing exit
            sma = SimpleMovingAverage(self.config.trail_period)
            self.sma10[iid] = sma
            self.register_indicator_for_bars(bt, sma)

            self.subscribe_bars(bt)

        total_signals = sum(len(v) for v in self._signals.values())
        self.log.info(
            f"SignalReplay started: {total_signals} signals, "
            f"{len(self.instruments)} instruments",
        )

    # -- bar handler ---------------------------------------------------------

    def on_bar(self, bar: Bar) -> None:
        iid = bar.bar_type.instrument_id
        if iid not in self.instruments:
            return

        self._diag_bars += 1

        # Log first few bar dates to verify timestamp parsing
        if len(self._diag_first_bar_dates) < 5:
            ticker = self._ticker_map[iid]
            if ticker == "BTCUSDT":
                date_str = pd.Timestamp(bar.ts_event, unit="ns").strftime("%Y-%m-%d")
                self._diag_first_bar_dates.append(
                    f"  ts_event={bar.ts_event}, date={date_str}, "
                    f"O={bar.open}, H={bar.high}, L={bar.low}, C={bar.close}"
                )

        # Manage existing position first
        if iid in self.entry_bar_count:
            self._diag_in_pos_skip += 1
            self._manage_position(bar, iid)
            return

        # Check for entry signal
        ticker = self._ticker_map[iid]
        date_str = pd.Timestamp(bar.ts_event, unit="ns").strftime("%Y-%m-%d")
        key = (date_str, ticker)
        stops = self._signals.get(key)
        if not stops:
            self._diag_no_signal += 1
            return

        # Take the first signal for this date/ticker, leave rest for later
        stop = stops.pop(0)
        if not stops:
            del self._signals[key]

        self._enter(bar, iid, stop)

    # -- entry ---------------------------------------------------------------

    def _enter(self, bar: Bar, iid: InstrumentId, stop_px: float) -> None:
        # Use bar open as fill proxy — Rust engine fills at next-day open,
        # and the CSV date IS the fill date (entry_row = N+1)
        fill_px = bar.open.as_double()
        stop_dist = abs(fill_px - stop_px)
        if stop_dist <= 0:
            self._diag_zero_dist += 1
            return

        # If stop is already above fill price (gap down), skip this trade
        if stop_px >= fill_px:
            self._diag_stop_above += 1
            return

        # Use fixed init_cash for sizing (Rust engine doesn't deplete equity)
        equity = self.config.init_cash

        risk_shares = (equity * self.config.risk_pct) / stop_dist
        max_shares = (equity * self.config.max_pos_pct) / fill_px
        shares = min(risk_shares, max_shares)
        if shares < 1:
            self._diag_no_equity += 1
            return

        instrument = self.instruments[iid]
        qty = instrument.make_qty(Decimal(str(int(shares))))
        if float(qty) <= 0:
            self._diag_zero_qty += 1
            return

        # Market buy only — no stop-market order
        entry_order = self.order_factory.market(
            instrument_id=iid,
            order_side=OrderSide.BUY,
            quantity=qty,
            time_in_force=TimeInForce.IOC,
        )
        self.submit_order(entry_order)

        self._diag_entries += 1
        self.entry_bar_count[iid] = 0
        self.entry_price[iid] = fill_px
        self.stop_price[iid] = stop_px

    # -- position management -------------------------------------------------

    def _manage_position(self, bar: Bar, iid: InstrumentId) -> None:
        count = self.entry_bar_count[iid] + 1
        self.entry_bar_count[iid] = count
        close = bar.close.as_double()

        # Rule 1: StopLoss — close <= stop price (checked first, highest priority)
        stop_px = self.stop_price.get(iid, 0.0)
        if close <= stop_px:
            self._diag_exits_stop += 1
            self._close_instrument(iid)
            return

        # Rule 2: ProfitTarget — exit full position after N bars if profitable
        if count >= self.config.profit_target_bars and close > self.entry_price[iid]:
            self._diag_exits_profit += 1
            self._close_instrument(iid)
            return

        # Rule 3: SmaCross — exit when close < SMA10
        sma = self.sma10.get(iid)
        if sma is not None and sma.initialized and sma.value > 0 and close < sma.value:
            self._diag_exits_sma += 1
            self._close_instrument(iid)
            return

    def _close_instrument(self, iid: InstrumentId) -> None:
        self.close_all_positions(iid)
        self._clear_tracking(iid)

    def _clear_tracking(self, iid: InstrumentId) -> None:
        self.entry_bar_count.pop(iid, None)
        self.entry_price.pop(iid, None)
        self.stop_price.pop(iid, None)

    def on_stop(self) -> None:
        remaining = sum(len(v) for v in self._signals.values())
        print("\n=== SIGNAL REPLAY DIAGNOSTICS ===")
        print(f"Bars processed:        {self._diag_bars}")
        print(f"Skipped (in position): {self._diag_in_pos_skip}")
        print(f"No signal match:       {self._diag_no_signal}")
        print(f"Signal matched:        {self._diag_entries + self._diag_stop_above + self._diag_zero_dist + self._diag_no_equity + self._diag_zero_qty}")
        print(f"  -> stop >= close:    {self._diag_stop_above}")
        print(f"  -> zero stop dist:   {self._diag_zero_dist}")
        print(f"  -> no equity (<1):   {self._diag_no_equity}")
        print(f"  -> zero qty:         {self._diag_zero_qty}")
        print(f"  -> ENTRIES:          {self._diag_entries}")
        print(f"Exits: stop={self._diag_exits_stop} profit={self._diag_exits_profit} sma={self._diag_exits_sma}")
        print(f"Signals remaining (unmatched): {remaining}")
        if self._diag_first_bar_dates:
            print("First BTCUSDT bar dates:")
            for d in self._diag_first_bar_dates:
                print(d)
        print("=================================\n")

    # -- events --------------------------------------------------------------

    def on_event(self, event) -> None:
        if isinstance(event, PositionClosed):
            iid = event.instrument_id
            self._clear_tracking(iid)


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(description="Replay Rust engine signals in NautilusTrader")
    parser.add_argument("--data-dir", type=Path, default=Path("data-crypto"))
    parser.add_argument("--trades", type=Path, required=True, help="CSV from --dump-trades")
    parser.add_argument("--init-cash", type=float, default=150)
    parser.add_argument("--log-level", default="INFO")
    args = parser.parse_args()

    data_dir = args.data_dir.resolve()

    # Read CSV to determine which tickers to load
    tickers: set[str] = set()
    with open(args.trades) as f:
        for row in csv.DictReader(f):
            tickers.add(row["ticker"])

    tickers_list = sorted(tickers)
    if "BTCUSDT" not in tickers_list:
        tickers_list.insert(0, "BTCUSDT")

    print(f"Loading data for {len(tickers_list)} tickers...")

    # Set up BacktestEngine
    engine = BacktestEngine(
        config=BacktestEngineConfig(
            trader_id=TraderId("REPLAY-001"),
            logging=LoggingConfig(log_level=args.log_level),
        ),
    )

    engine.add_venue(
        venue=BINANCE_VENUE,
        oms_type=OmsType.NETTING,
        account_type=AccountType.MARGIN,
        base_currency=None,
        starting_balances=[Money(args.init_cash, USDT)],
        bar_execution=True,
    )

    instrument_ids = []
    t0 = time.time()

    for i, symbol in enumerate(tickers_list):
        df = load_ticker_csv(data_dir, symbol, "1d")
        if df is None or df.empty:
            print(f"  [{i+1}/{len(tickers_list)}] {symbol}: no data, skipping")
            continue

        instrument = make_instrument(symbol)
        engine.add_instrument(instrument)

        bt = BarType.from_str(f"{symbol}.BINANCE-1-DAY-LAST-EXTERNAL")
        wrangler = BarDataWrangler(bt, instrument)
        bars = wrangler.process(df)
        engine.add_data(bars, sort=False)

        instrument_ids.append(f"{symbol}.BINANCE")
        print(f"  [{i+1}/{len(tickers_list)}] {symbol}: {len(bars)} bars loaded")

    if not instrument_ids:
        print("Error: no data loaded", file=sys.stderr)
        sys.exit(1)

    print(f"\nData loaded in {time.time() - t0:.1f}s")
    engine.sort_data()

    # Create strategy
    config = SignalReplayConfig(
        instrument_ids=instrument_ids,
        bar_spec="1-DAY-LAST",
        trades_csv=str(args.trades),
        init_cash=args.init_cash,
    )
    strategy = SignalReplayStrategy(config=config)
    engine.add_strategy(strategy)

    print(f"\nRunning signal replay with {len(instrument_ids)} instruments...")
    t1 = time.time()
    engine.run()
    elapsed = time.time() - t1
    print(f"\nBacktest completed in {elapsed:.1f}s")

    print("\n--- Account Report ---")
    print(engine.trader.generate_account_report(BINANCE_VENUE))
    print("\n--- Order Fills ---")
    print(engine.trader.generate_order_fills_report())
    print("\n--- Positions ---")
    print(engine.trader.generate_positions_report())

    engine.dispose()


if __name__ == "__main__":
    main()
