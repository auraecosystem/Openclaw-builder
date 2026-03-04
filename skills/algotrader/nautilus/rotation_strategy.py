"""Cross-sectional rotational momentum strategy for NautilusTrader.

Ranks all instruments by a precomputed composite score (RS momentum +
pattern quality + signal features), holds the top N, rebalances on a
fixed bar schedule, and sizes positions with ATR-based risk targeting
compounding portfolio equity.

Scores are precomputed offline (lib/build_cache.py + lib/score_builder.py)
and loaded as numpy arrays at strategy start. The strategy reads
score[date_idx, ticker_idx] — no indicator computation at runtime.
"""

from __future__ import annotations

import math
from decimal import Decimal
from pathlib import Path

import numpy as np
import pandas as pd

from nautilus_trader.common.enums import LogColor
from nautilus_trader.config import StrategyConfig
from nautilus_trader.model.data import Bar, BarType
from nautilus_trader.model.enums import OrderSide, TimeInForce
from nautilus_trader.model.identifiers import InstrumentId, Venue
from nautilus_trader.model.objects import Currency, Quantity
from nautilus_trader.trading.strategy import Strategy


class RotationConfig(StrategyConfig, frozen=True):
    """Parameters for the cross-sectional rotation strategy."""

    instrument_ids: list[str]
    bar_spec: str = "1-DAY-LAST"
    cache_dir: str = "data/cache"

    # Venue settings
    venue: str = "XNYS"
    quote_currency: str = "USD"

    # Rotation parameters
    top_n: int = 20                 # max concurrent positions
    rebalance_bars: int = 5         # rebalance every N bars
    min_score: float = 0.0          # minimum composite score for entry

    # Composite score weights (must sum to ~1.0 for interpretability)
    w_rs: float = 0.4               # RS percentile rank weight
    w_pattern: float = 0.3          # pattern quality score weight
    w_signal: float = 0.3           # signal pipeline score weight

    # Risk management
    risk_pct: float = 0.01          # fraction of equity risked per position
    max_pos_pct: float = 0.10       # max single position as fraction of equity
    stop_atr_mult: float = 2.0      # stop distance in ATR multiples


class RotationStrategy(Strategy):
    """Cross-sectional rotational momentum with precomputed scores.

    Rebalancing logic:
    1. Every rebalance_bars bars, compute composite score for all instruments
       using preloaded arrays indexed by current date.
    2. Rank instruments. Top N = target portfolio.
    3. Exit positions not in top N.
    4. Enter positions in top N not currently held.
    5. Size by compounding equity * risk_pct / (stop_atr_mult * ATR).
    """

    def __init__(self, config: RotationConfig) -> None:
        super().__init__(config)

        self.bar_types: dict[InstrumentId, BarType] = {}
        self.instruments: dict[InstrumentId, object] = {}

        # Score arrays: (n_dates, n_tickers) float32
        self._rs_scores: np.ndarray | None = None
        self._pattern_scores: np.ndarray | None = None
        self._signal_scores: np.ndarray | None = None
        self._atr_arr: np.ndarray | None = None

        # Index mappings
        self._ticker_to_col: dict[str, int] = {}
        self._iid_to_col: dict[InstrumentId, int] = {}
        self._dates: np.ndarray | None = None  # sorted date array for bisect
        self._date_to_row: dict[int, int] = {}  # date ordinal → row index

        # Bar tracking
        self._bar_count: int = 0
        self._last_prices: dict[InstrumentId, float] = {}

    def on_start(self) -> None:
        cfg = self.config
        self._venue = Venue(cfg.venue)
        self._quote_currency = Currency.from_str(cfg.quote_currency)
        cache_dir = Path(cfg.cache_dir)

        # Load score arrays
        self._load_scores(cache_dir)

        # Subscribe to bars for all instruments
        for iid_str in cfg.instrument_ids:
            iid = InstrumentId.from_str(iid_str)
            instrument = self.cache.instrument(iid)
            if instrument is None:
                self.log.warning(f"Instrument {iid} not found, skipping")
                continue

            self.instruments[iid] = instrument
            bt = BarType.from_str(f"{iid_str}-{cfg.bar_spec}-EXTERNAL")
            self.bar_types[iid] = bt

            # Map instrument to column index in score arrays
            ticker = iid.symbol.value
            if ticker in self._ticker_to_col:
                self._iid_to_col[iid] = self._ticker_to_col[ticker]

            self.subscribe_bars(bt)

        self.log.info(
            f"Started rotation: {len(self.instruments)} instruments, "
            f"top_n={cfg.top_n}, rebalance every {cfg.rebalance_bars} bars",
            color=LogColor.BLUE,
        )

    def _load_scores(self, cache_dir: Path) -> None:
        """Load precomputed score arrays and build index mappings."""
        # Load RS scores (always available after Phase 0)
        rs_path = cache_dir / "rs_score.npy"
        if rs_path.exists():
            self._rs_scores = np.load(rs_path)

        # Load pattern scores
        pat_path = cache_dir / "pattern_score.npy"
        if pat_path.exists():
            self._pattern_scores = np.load(pat_path)

        # Load signal scores (optional — requires Phase 1 PyO3 bridge)
        sig_path = cache_dir / "signal_score.npy"
        if sig_path.exists():
            self._signal_scores = np.load(sig_path)

        # Load ATR for position sizing
        atr_path = cache_dir / "atr_14.parquet"
        if atr_path.exists():
            atr_df = pd.read_parquet(atr_path)
            self._atr_arr = atr_df.values.astype(np.float32)
            # Build ticker → column index mapping from parquet columns
            for i, col in enumerate(atr_df.columns):
                self._ticker_to_col[str(col)] = i
            # Build date → row index mapping
            if hasattr(atr_df.index, 'date'):
                for i, dt in enumerate(atr_df.index):
                    self._date_to_row[dt.toordinal()] = i
            else:
                # Fallback: sequential indexing
                for i in range(len(atr_df)):
                    self._date_to_row[i] = i

        n_scores = sum(1 for s in [self._rs_scores, self._pattern_scores, self._signal_scores] if s is not None)
        self.log.info(f"Loaded {n_scores}/3 score arrays, ATR={'yes' if self._atr_arr is not None else 'no'}")

    def on_bar(self, bar: Bar) -> None:
        iid = bar.bar_type.instrument_id
        if iid not in self.instruments:
            return

        self._last_prices[iid] = bar.close.as_double()
        self._bar_count += 1

        if self._bar_count % self.config.rebalance_bars == 0:
            self._rebalance()

    def _get_row_index(self) -> int | None:
        """Get current row index in score arrays from bar count.

        Since all instruments receive bars in chronological order and we
        count total bars / n_instruments, we approximate the date row.
        For exact mapping, use timestamp-based lookup.
        """
        n_inst = max(len(self.instruments), 1)
        return self._bar_count // n_inst

    def _composite_scores(self, row: int) -> np.ndarray:
        """Compute composite score vector for all tickers at given row."""
        cfg = self.config
        n_cols = len(self._ticker_to_col)
        scores = np.zeros(n_cols, dtype=np.float32)

        if self._rs_scores is not None and row < self._rs_scores.shape[0]:
            rs = self._rs_scores[row]
            # Normalize NaN to 0
            rs = np.nan_to_num(rs, nan=0.0)
            scores += cfg.w_rs * rs

        if self._pattern_scores is not None and row < self._pattern_scores.shape[0]:
            pat = self._pattern_scores[row]
            pat = np.nan_to_num(pat, nan=0.0)
            scores += cfg.w_pattern * pat

        if self._signal_scores is not None and row < self._signal_scores.shape[0]:
            sig = self._signal_scores[row]
            sig = np.nan_to_num(sig, nan=0.0)
            scores += cfg.w_signal * sig

        return scores

    def _rebalance(self) -> None:
        """Core rebalance: rank, diff vs current, enter/exit."""
        cfg = self.config
        row = self._get_row_index()
        if row is None:
            return

        scores = self._composite_scores(row)

        # Find top N tickers above minimum score
        above_min = np.where(scores > cfg.min_score)[0]
        if len(above_min) == 0:
            # Close everything — no qualifying instruments
            self._exit_all()
            return

        # Sort by score descending, take top N
        sorted_cols = above_min[np.argsort(scores[above_min])[::-1]]
        target_cols = set(sorted_cols[:cfg.top_n].tolist())

        # Map column indices to instrument IDs
        col_to_iid: dict[int, InstrumentId] = {}
        for iid, col in self._iid_to_col.items():
            col_to_iid[col] = iid

        target_iids = {col_to_iid[c] for c in target_cols if c in col_to_iid}

        # Current positions
        current_positions = set()
        for iid in self.instruments:
            positions = self.cache.positions(venue=self._venue, instrument_id=iid)
            if positions and any(p.is_open for p in positions):
                current_positions.add(iid)

        # Exit positions not in target
        to_exit = current_positions - target_iids
        for iid in to_exit:
            self.close_all_positions(iid)
            self.cancel_all_orders(iid)

        # Enter new targets not currently held
        to_enter = target_iids - current_positions
        if not to_enter:
            return

        # Get current equity for sizing
        account = self.portfolio.account(self._venue)
        equity = float(account.balance_total(self._quote_currency))
        if equity <= 0:
            return

        for iid in to_enter:
            col = self._iid_to_col.get(iid)
            if col is None:
                continue

            close = self._last_prices.get(iid, 0.0)
            if close <= 0:
                continue

            # ATR for stop distance
            atr = self._get_atr(row, col)
            if atr <= 0 or math.isnan(atr):
                continue

            stop_dist = cfg.stop_atr_mult * atr
            if stop_dist <= 0:
                continue

            # Risk-based sizing with position cap
            risk_shares = (equity * cfg.risk_pct) / stop_dist
            max_shares = (equity * cfg.max_pos_pct) / close
            shares = int(min(risk_shares, max_shares))
            if shares < 1:
                continue

            instrument = self.instruments[iid]
            qty = instrument.make_qty(Decimal(str(shares)))
            if float(qty) <= 0:
                continue

            # Market entry order
            entry = self.order_factory.market(
                instrument_id=iid,
                order_side=OrderSide.BUY,
                quantity=qty,
                time_in_force=TimeInForce.IOC,
            )
            self.submit_order(entry)

            # Stop-loss
            stop_price = close - stop_dist
            if stop_price > 0:
                stop = self.order_factory.stop_market(
                    instrument_id=iid,
                    order_side=OrderSide.SELL,
                    quantity=qty,
                    trigger_price=instrument.make_price(stop_price),
                    time_in_force=TimeInForce.GTC,
                    reduce_only=True,
                )
                self.submit_order(stop)

        if to_exit or to_enter:
            self.log.info(
                f"Rebalance: entered {len(to_enter)}, exited {len(to_exit)}, "
                f"holding {len(target_iids & current_positions) + len(to_enter)} positions",
                color=LogColor.BLUE,
            )

    def _get_atr(self, row: int, col: int) -> float:
        """Read ATR value from precomputed array."""
        if self._atr_arr is None:
            return 0.0
        if row >= self._atr_arr.shape[0] or col >= self._atr_arr.shape[1]:
            return 0.0
        val = self._atr_arr[row, col]
        return 0.0 if math.isnan(val) else float(val)

    def _exit_all(self) -> None:
        """Close all open positions."""
        for iid in self.instruments:
            self.close_all_positions(iid)
            self.cancel_all_orders(iid)

    def on_stop(self) -> None:
        for iid in list(self.instruments):
            self.cancel_all_orders(iid)
            self.close_all_positions(iid)

    def on_reset(self) -> None:
        self._bar_count = 0
        self._last_prices.clear()
