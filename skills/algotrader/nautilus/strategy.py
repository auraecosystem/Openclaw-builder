"""Qullamaggie breakout swing trading strategy for NautilusTrader.

Implements a multi-instrument breakout strategy based on Mark Minervini /
Qullamaggie principles: relative strength ranking, VCP and bull flag pattern
detection, volume confirmation, and risk-based position sizing with trailing
SMA exits.

A single Strategy instance manages all instruments. Regime filtering uses
BTC EMA crossover to avoid trading during broad downtrends.

Two variants:
  - QullamaggieBreakout: computes indicators at runtime (standalone use)
  - PrecomputedBreakout: reads pre-computed numpy arrays (evolutionary optimizer)
"""

from __future__ import annotations

import math
from decimal import Decimal

import numpy as np

from nautilus_trader.common.enums import LogColor
from nautilus_trader.config import StrategyConfig
from nautilus_trader.indicators import AverageTrueRange
from nautilus_trader.indicators import ExponentialMovingAverage
from nautilus_trader.indicators import SimpleMovingAverage
from nautilus_trader.model.currencies import USD
from nautilus_trader.model.data import Bar, BarType
from nautilus_trader.model.enums import OrderSide, TimeInForce
from nautilus_trader.model.identifiers import InstrumentId, Venue
from nautilus_trader.model.objects import Currency, Price, Quantity
from nautilus_trader.trading.strategy import Strategy

from nautilus.indicators import (
    ConsecutiveGreen,
    FlagDetector,
    RollingHigh,
    RollingReturn,
    VCPDetector,
    VolumeSMA,
)


# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------


class QullamaggieConfig(StrategyConfig, frozen=True):
    """Parameters for the Qullamaggie breakout strategy."""

    instrument_ids: list[str]
    bar_spec: str = "1-DAY-LAST"

    # Venue settings (defaults to Binance for backward compatibility)
    venue: str = "BINANCE"
    benchmark_id: str = "BTCUSDT.BINANCE"  # regime filter instrument
    quote_currency: str = "USDT"           # account balance currency

    # Entry filters
    rs_pct: float = 0.30          # top N% relative strength (63d return rank)
    max_dist_52w: float = 0.25    # max distance from 52-week high
    min_prior_move: float = 0.30  # minimum 63-bar return before base
    min_adr_pct: float = 0.03     # minimum ATR / price ratio
    vol_spike: float = 1.5        # volume must exceed N * 20d avg

    # Pattern filter thresholds are embedded in the entry logic; the config
    # controls consolidation tightness for VCP only.
    max_range_pct: float = 0.15

    # Indicator periods (in days — multiplied by bars_per_day internally)
    sma_fast: int = 10            # fast SMA period
    sma_slow: int = 20            # slow SMA period
    atr_period: int = 14          # ATR period
    vol_sma_period: int = 20      # volume SMA period
    ret_short: int = 21           # short return lookback
    ret_medium: int = 63          # medium return lookback
    ret_long: int = 126           # long return lookback
    regime_ema_fast: int = 10     # regime filter fast EMA
    regime_ema_slow: int = 20     # regime filter slow EMA

    # Pattern detection
    pattern_lookback: int = 60    # VCP/flag lookback window
    swing_order: int = 5          # swing detection order for VCP/flag
    max_pole_bars: int = 20       # max pole length for flag detection
    flag_min_pole: float = 0.20   # min pole move for flag pattern
    flag_max_retrace: float = 0.50  # max retrace for flag pattern
    flag_min_days: int = 5        # min flag duration
    flag_max_days: int = 25       # max flag duration

    # Risk management
    risk_pct: float = 0.005       # fraction of equity risked per trade
    max_pos_pct: float = 0.20     # max position as fraction of equity
    liq_cap_pct: float = 0.01     # max fraction of avg daily volume to take
    partial_bars: int = 5         # bars before partial profit is taken
    trail_period: int = 10        # SMA period for trailing exit (10 or 20)
    split_frac: float = 0.50      # partial exit fraction (0.0 = no partial, 1.0 = sell all)
    max_hold_bars: int = 0        # time stop: close after N bars (0 = disabled)

    # Lookbacks
    rs_lookback: int = 63         # return period for RS ranking
    bars_per_day: int = 1         # 1=daily, 288=5m, 24=1h — scales all periods
    high_lookback: int = 252      # rolling high period in days (252=1yr, 30=1mo)


# ---------------------------------------------------------------------------
# Strategy
# ---------------------------------------------------------------------------


class QullamaggieBreakout(Strategy):
    """Multi-instrument Qullamaggie breakout strategy.

    Entry criteria (all must be true):
        - Regime: benchmark EMA10 >= EMA20
        - Relative strength: 63d return in top ``rs_pct`` percentile
        - Within ``max_dist_52w`` of 52-week high
        - Prior move: 63-bar return >= ``min_prior_move``
        - ADR (ATR/price) >= ``min_adr_pct``
        - VCP or bull flag pattern detected
        - Volume spike >= ``vol_spike`` * 20d average

    Exit logic:
        - Hard stop-loss at entry low minus ATR
        - Partial profit (50%) after ``partial_bars``, stop moved to breakeven
        - Trailing SMA exit when close < SMA(trail_period)

    All period params are in "days" units. Set ``bars_per_day`` to scale
    indicator lookbacks for sub-daily bars (288 for 5m, 24 for 1h).
    """

    def __init__(self, config: QullamaggieConfig) -> None:
        super().__init__(config)

        # Per-instrument built-in indicators
        self.sma10: dict[InstrumentId, SimpleMovingAverage] = {}
        self.sma20: dict[InstrumentId, SimpleMovingAverage] = {}
        self.atr14: dict[InstrumentId, AverageTrueRange] = {}

        # Per-instrument custom indicators
        self.vol_sma: dict[InstrumentId, VolumeSMA] = {}
        self.rolling_high: dict[InstrumentId, RollingHigh] = {}
        self.ret_21: dict[InstrumentId, RollingReturn] = {}
        self.ret_63: dict[InstrumentId, RollingReturn] = {}
        self.ret_126: dict[InstrumentId, RollingReturn] = {}
        self.ret_rs: dict[InstrumentId, RollingReturn] = {}
        self.consec_green: dict[InstrumentId, ConsecutiveGreen] = {}
        self.vcp: dict[InstrumentId, VCPDetector] = {}
        self.flag: dict[InstrumentId, FlagDetector] = {}

        # Instrument / bar-type lookups
        self.bar_types: dict[InstrumentId, BarType] = {}
        self.instruments: dict[InstrumentId, object] = {}

        # Trade tracking (only present while in a position)
        self.entry_bar_count: dict[InstrumentId, int] = {}
        self.entry_price: dict[InstrumentId, float] = {}
        self.stop_orders: dict[InstrumentId, object] = {}
        self.partial_taken: dict[InstrumentId, bool] = {}

        # Regime filter (benchmark EMA crossover)
        self.benchmark_id: InstrumentId | None = None
        self.regime_ok: bool = False
        self.regime_ema10: ExponentialMovingAverage | None = None
        self.regime_ema20: ExponentialMovingAverage | None = None

    # -- lifecycle -----------------------------------------------------------

    def on_start(self) -> None:
        cfg = self.config
        self.benchmark_id = InstrumentId.from_str(cfg.benchmark_id)
        self._venue = Venue(cfg.venue)
        self._quote_currency = Currency.from_str(cfg.quote_currency)
        bpd = cfg.bars_per_day

        for iid_str in cfg.instrument_ids:
            iid = InstrumentId.from_str(iid_str)
            instrument = self.cache.instrument(iid)
            if instrument is None:
                self.log.error(f"Instrument {iid} not found in cache, skipping")
                continue

            self.instruments[iid] = instrument

            bt = BarType.from_str(f"{iid_str}-{self.config.bar_spec}-EXTERNAL")
            self.bar_types[iid] = bt

            # Built-in indicators (periods scaled by bars_per_day)
            cfg = self.config
            sma10 = SimpleMovingAverage(cfg.sma_fast * bpd)
            sma20 = SimpleMovingAverage(cfg.sma_slow * bpd)
            atr14 = AverageTrueRange(cfg.atr_period * bpd)
            self.sma10[iid] = sma10
            self.sma20[iid] = sma20
            self.atr14[iid] = atr14

            # Custom indicators (periods scaled by bars_per_day)
            vol_sma = VolumeSMA(cfg.vol_sma_period * bpd)
            rolling_high = RollingHigh(cfg.high_lookback * bpd)
            ret_21 = RollingReturn(cfg.ret_short * bpd)
            ret_63 = RollingReturn(cfg.ret_medium * bpd)
            ret_126 = RollingReturn(cfg.ret_long * bpd)
            ret_rs = RollingReturn(cfg.rs_lookback * bpd)
            consec_green = ConsecutiveGreen()
            # Pattern detectors: lookback stays small (O(lookback²) per bar).
            vcp_det = VCPDetector(lookback=cfg.pattern_lookback, swing_order=cfg.swing_order)
            flag_det = FlagDetector(
                lookback=cfg.pattern_lookback, swing_order=cfg.swing_order,
                max_flag_bars=cfg.flag_max_days, max_pole_bars=cfg.max_pole_bars,
            )

            self.vol_sma[iid] = vol_sma
            self.rolling_high[iid] = rolling_high
            self.ret_21[iid] = ret_21
            self.ret_63[iid] = ret_63
            self.ret_126[iid] = ret_126
            self.ret_rs[iid] = ret_rs
            self.consec_green[iid] = consec_green
            self.vcp[iid] = vcp_det
            self.flag[iid] = flag_det

            # Register all indicators for this bar type
            for ind in (
                sma10, sma20, atr14,
                vol_sma, rolling_high,
                ret_21, ret_63, ret_126, ret_rs,
                consec_green, vcp_det, flag_det,
            ):
                self.register_indicator_for_bars(bt, ind)

            self.subscribe_bars(bt)

        # Regime EMAs — created after the loop so instrument ordering doesn't matter
        if self.benchmark_id in self.bar_types:
            bt = self.bar_types[self.benchmark_id]
            self.regime_ema10 = ExponentialMovingAverage(cfg.regime_ema_fast * bpd)
            self.regime_ema20 = ExponentialMovingAverage(cfg.regime_ema_slow * bpd)
            self.register_indicator_for_bars(bt, self.regime_ema10)
            self.register_indicator_for_bars(bt, self.regime_ema20)
        else:
            self.log.warning(
                f"Benchmark {self.benchmark_id} not in instrument list — "
                "regime filter disabled (always True)",
                color=LogColor.YELLOW,
            )
            self.regime_ok = True

        self.log.info(
            f"Started with {len(self.instruments)} instruments, "
            f"benchmark={self.benchmark_id}",
            color=LogColor.BLUE,
        )

    # -- bar handler ---------------------------------------------------------

    def on_bar(self, bar: Bar) -> None:
        iid = bar.bar_type.instrument_id
        if iid not in self.instruments:
            return

        # Update regime filter on benchmark bars
        if (
            iid == self.benchmark_id
            and self.regime_ema10 is not None
            and self.regime_ema10.initialized
        ):
            self.regime_ok = self.regime_ema10.value >= self.regime_ema20.value

        # If we hold a position for this instrument, manage it
        if iid in self.entry_bar_count:
            self._manage_position(bar, iid)
            return

        # Otherwise evaluate for new entry
        if self.regime_ok:
            self._evaluate_entry(bar, iid)

    # -- position management -------------------------------------------------

    def _manage_position(self, bar: Bar, iid: InstrumentId) -> None:
        """Handle partial profit, trailing SMA exit, and bar counting."""
        count = self.entry_bar_count[iid] + 1
        self.entry_bar_count[iid] = count
        close = bar.close.as_double()

        # Partial profit after N bars if in profit
        if (
            count >= self.config.partial_bars
            and close > self.entry_price[iid]
            and not self.partial_taken[iid]
        ):
            positions = self.cache.positions(
                venue=self._venue, instrument_id=iid,
            )
            if positions and positions[0].is_open:
                sell_qty = int(float(positions[0].quantity) * self.config.split_frac)
                if sell_qty > 0:
                    instrument = self.instruments[iid]
                    qty = instrument.make_qty(Decimal(str(sell_qty)))
                    order = self.order_factory.market(
                        instrument_id=iid,
                        order_side=OrderSide.SELL,
                        quantity=qty,
                    )
                    self.submit_order(order)
                    self.partial_taken[iid] = True

                    # Move stop to breakeven
                    stop = self.stop_orders.get(iid)
                    if stop is not None:
                        be_price = instrument.make_price(self.entry_price[iid])
                        self.modify_order(stop, trigger_price=be_price)

        # Time stop
        if self.config.max_hold_bars > 0 and count >= self.config.max_hold_bars:
            self._close_instrument(iid)
            return

        # Trailing SMA exit
        trail_sma = (
            self.sma10[iid] if self.config.trail_period == 10
            else self.sma20[iid]
        )
        sma_val = trail_sma.value
        if sma_val > 0 and close < sma_val and trail_sma.initialized:
            self._close_instrument(iid)

    def _close_instrument(self, iid: InstrumentId) -> None:
        """Flatten position, cancel stop, clear tracking for one instrument."""
        self.close_all_positions(iid)

        stop = self.stop_orders.get(iid)
        if stop is not None:
            self.cancel_order(stop)
            self.stop_orders[iid] = None

        self._clear_tracking(iid)

    def _clear_tracking(self, iid: InstrumentId) -> None:
        self.entry_bar_count.pop(iid, None)
        self.entry_price.pop(iid, None)
        self.stop_orders.pop(iid, None)
        self.partial_taken.pop(iid, None)

    # -- entry evaluation ----------------------------------------------------

    def _evaluate_entry(self, bar: Bar, iid: InstrumentId) -> None:
        """Screen a single instrument for breakout entry on this bar."""
        # All indicators must be warmed up
        if not (
            self.sma10[iid].initialized
            and self.sma20[iid].initialized
            and self.atr14[iid].initialized
            and self.vol_sma[iid].initialized
            and self.rolling_high[iid].initialized
            and self.ret_63[iid].initialized
            and self.vcp[iid].initialized
            and self.flag[iid].initialized
        ):
            return

        close = bar.close.as_double()

        # -- Relative strength percentile (configurable lookback) -----------
        returns_rs = {
            k: v.value for k, v in self.ret_rs.items() if v.initialized
        }
        if not returns_rs:
            return
        my_ret = returns_rs.get(iid, 0.0)
        rank = sum(1 for v in returns_rs.values() if v <= my_ret) / len(returns_rs)
        if rank < (1.0 - self.config.rs_pct):
            return

        # -- Distance from 52-week high -------------------------------------
        rh = self.rolling_high[iid]
        if rh.value > 0 and (rh.value - close) / rh.value > self.config.max_dist_52w:
            return

        # -- Prior move (63-bar return) --------------------------------------
        if self.ret_63[iid].value < self.config.min_prior_move:
            return

        # -- ADR check -------------------------------------------------------
        atr_val = self.atr14[iid].value
        if close > 0 and atr_val / close < self.config.min_adr_pct:
            return

        # -- Pattern detection -----------------------------------------------
        vcp_ind = self.vcp[iid]
        vcp_signal = (
            vcp_ind.num_contractions >= 2
            and vcp_ind.last_contraction_pct < self.config.max_range_pct
            and vcp_ind.tightening_ratio < 1.0
            and vcp_ind.vol_trend < 1.0
        )

        flg = self.flag[iid]
        bpd = self.config.bars_per_day
        flag_signal = (
            flg.pole_pct >= self.config.flag_min_pole
            and 0.0 <= flg.retrace_pct < self.config.flag_max_retrace
            and self.config.flag_min_days * bpd <= flg.flag_days <= self.config.flag_max_days * bpd
            and flg.vol_ratio < 1.0
        )

        if not (vcp_signal or flag_signal):
            return

        # -- Volume spike ----------------------------------------------------
        vol = bar.volume.as_double()
        vsma = self.vol_sma[iid].value
        if vsma > 0 and vol < self.config.vol_spike * vsma:
            return

        # -- Position sizing -------------------------------------------------
        account = self.portfolio.account(self._venue)
        equity = float(account.balance_total(self._quote_currency))

        stop_price = max(bar.low.as_double(), close - atr_val)
        stop_dist = abs(close - stop_price)
        if stop_dist <= 0:
            return

        risk_shares = (equity * self.config.risk_pct) / stop_dist
        max_shares = (equity * self.config.max_pos_pct) / close

        # Liquidity cap: limit to configured fraction of average daily volume
        adv = vsma * close
        liq_cap = (self.config.liq_cap_pct * adv) / close if adv > 0 else risk_shares

        shares = min(risk_shares, max_shares, liq_cap)
        if shares < 1:
            return

        instrument = self.instruments[iid]
        qty = instrument.make_qty(Decimal(str(int(shares))))
        if float(qty) <= 0:
            return

        # -- Submit entry + stop orders --------------------------------------
        entry_order = self.order_factory.market(
            instrument_id=iid,
            order_side=OrderSide.BUY,
            quantity=qty,
            time_in_force=TimeInForce.IOC,
        )
        self.submit_order(entry_order)

        stop_order = self.order_factory.stop_market(
            instrument_id=iid,
            order_side=OrderSide.SELL,
            quantity=qty,
            trigger_price=instrument.make_price(stop_price),
            time_in_force=TimeInForce.GTC,
            reduce_only=True,
        )
        self.submit_order(stop_order)

        # -- Track the new position ------------------------------------------
        self.entry_bar_count[iid] = 0
        self.entry_price[iid] = close
        self.stop_orders[iid] = stop_order
        self.partial_taken[iid] = False

        pattern = "VCP" if vcp_signal else "FLAG"
        self.log.info(
            f"ENTRY {iid}: qty={qty}, stop={stop_price:.4f}, pattern={pattern}",
            color=LogColor.GREEN,
        )

    # -- events --------------------------------------------------------------

    def on_event(self, event) -> None:
        # Clean up tracking when a stop fill or position close event arrives
        # for an instrument we are tracking.
        iid: InstrumentId | None = getattr(event, "instrument_id", None)
        if iid is None:
            return

        if iid not in self.entry_bar_count:
            return

        from nautilus_trader.model.events import OrderFilled, PositionClosed

        if isinstance(event, PositionClosed):
            self._clear_tracking(iid)
        elif isinstance(event, OrderFilled):
            # If the filled order is our stop, clean up
            stop = self.stop_orders.get(iid)
            if stop is not None and event.client_order_id == stop.client_order_id:
                self._clear_tracking(iid)

    # -- shutdown / reset ----------------------------------------------------

    def on_stop(self) -> None:
        for iid in list(self.instruments):
            self.cancel_all_orders(iid)
            self.close_all_positions(iid)

    def on_reset(self) -> None:
        for dct in (
            self.sma10, self.sma20, self.atr14,
            self.vol_sma, self.rolling_high,
            self.ret_21, self.ret_63, self.ret_126, self.ret_rs,
            self.consec_green, self.vcp, self.flag,
        ):
            for indicator in dct.values():
                indicator.reset()

        if self.regime_ema10 is not None:
            self.regime_ema10.reset()
        if self.regime_ema20 is not None:
            self.regime_ema20.reset()

        self.entry_bar_count.clear()
        self.entry_price.clear()
        self.stop_orders.clear()
        self.partial_taken.clear()
        self.regime_ok = False


# ---------------------------------------------------------------------------
# PrecomputedConfig — same params, used by PrecomputedBreakout
# ---------------------------------------------------------------------------


class PrecomputedConfig(StrategyConfig, frozen=True):
    """Parameters for the pre-computed breakout strategy (same as Qullamaggie)."""

    instrument_ids: list[str]
    bar_spec: str = "1-DAY-LAST"

    # Venue settings
    venue: str = "BINANCE"
    benchmark_id: str = "BTCUSDT.BINANCE"
    quote_currency: str = "USDT"

    # Entry filters
    rs_pct: float = 0.30
    max_dist_52w: float = 0.25
    min_prior_move: float = 0.30
    min_adr_pct: float = 0.03
    vol_spike: float = 1.5
    max_range_pct: float = 0.15

    # Risk management
    risk_pct: float = 0.005
    max_pos_pct: float = 0.20
    liq_cap_pct: float = 0.01
    partial_bars: int = 5
    trail_period: int = 10
    split_frac: float = 0.50
    max_hold_bars: int = 0
    flag_min_pole: float = 0.20
    flag_max_retrace: float = 0.50
    flag_min_days: int = 5
    flag_max_days: int = 25
    rs_lookback: int = 63
    bars_per_day: int = 1         # 1=daily, 288=5m, 24=1h — scales all periods
    high_lookback: int = 252      # rolling high period in days (252=1yr, 30=1mo)


# ---------------------------------------------------------------------------
# PrecomputedBreakout — reads pre-computed numpy arrays, zero indicator work
# ---------------------------------------------------------------------------


class PrecomputedBreakout(Strategy):
    """Same trading logic as QullamaggieBreakout but reads pre-computed arrays.

    Designed for the evolutionary optimizer where indicators are identical
    across all parameter combinations. Each worker pre-computes indicator
    values once, then this strategy just indexes into numpy arrays.
    """

    def __init__(
        self,
        config: PrecomputedConfig,
        indicators: dict[str, dict[str, np.ndarray]],
    ) -> None:
        super().__init__(config)
        self._indicators = indicators

        self.bar_types: dict[InstrumentId, BarType] = {}
        self.instruments: dict[InstrumentId, object] = {}

        # Per-instrument bar counter (index into pre-computed arrays)
        self._bar_idx: dict[InstrumentId, int] = {}
        self._ticker_map: dict[InstrumentId, str] = {}

        # Trade tracking
        self.entry_bar_count: dict[InstrumentId, int] = {}
        self.entry_price: dict[InstrumentId, float] = {}
        self.stop_orders: dict[InstrumentId, object] = {}
        self.partial_taken: dict[InstrumentId, bool] = {}

        self.benchmark_id: InstrumentId | None = None
        self.regime_ok: bool = False

    # -- lifecycle -----------------------------------------------------------

    def on_start(self) -> None:
        self.benchmark_id = InstrumentId.from_str(self.config.benchmark_id)
        self._venue = Venue(self.config.venue)
        self._quote_currency = Currency.from_str(self.config.quote_currency)

        for iid_str in self.config.instrument_ids:
            iid = InstrumentId.from_str(iid_str)
            instrument = self.cache.instrument(iid)
            if instrument is None:
                continue

            self.instruments[iid] = instrument
            bt = BarType.from_str(f"{iid_str}-{self.config.bar_spec}-EXTERNAL")
            self.bar_types[iid] = bt
            self._bar_idx[iid] = -1

            ticker = iid.symbol.value
            self._ticker_map[iid] = ticker

            self.subscribe_bars(bt)

    # -- bar handler ---------------------------------------------------------

    def on_bar(self, bar: Bar) -> None:
        iid = bar.bar_type.instrument_id
        if iid not in self.instruments:
            return

        idx = self._bar_idx[iid] + 1
        self._bar_idx[iid] = idx
        ticker = self._ticker_map[iid]
        ind = self._indicators.get(ticker)
        if ind is None:
            return

        # Regime filter from benchmark EMA crossover
        if iid == self.benchmark_id and idx < len(ind["ema10"]):
            e10 = ind["ema10"][idx]
            e20 = ind["ema20"][idx]
            if not (math.isnan(e10) or math.isnan(e20)):
                self.regime_ok = e10 >= e20

        # Manage existing positions
        if iid in self.entry_bar_count:
            self._manage_position(bar, iid, idx, ind)
            return

        # Evaluate new entries
        warmup = self.config.high_lookback * self.config.bars_per_day
        if self.regime_ok and idx >= warmup:
            self._evaluate_entry(bar, iid, idx, ind)

    # -- position management -------------------------------------------------

    def _manage_position(
        self, bar: Bar, iid: InstrumentId, idx: int, ind: dict[str, np.ndarray],
    ) -> None:
        count = self.entry_bar_count[iid] + 1
        self.entry_bar_count[iid] = count
        close = bar.close.as_double()

        # Partial profit after N bars if in profit
        if (
            count >= self.config.partial_bars
            and close > self.entry_price[iid]
            and not self.partial_taken[iid]
        ):
            positions = self.cache.positions(
                venue=self._venue, instrument_id=iid,
            )
            if positions and positions[0].is_open:
                sell_qty = int(float(positions[0].quantity) * self.config.split_frac)
                if sell_qty > 0:
                    instrument = self.instruments[iid]
                    qty = instrument.make_qty(Decimal(str(sell_qty)))
                    order = self.order_factory.market(
                        instrument_id=iid,
                        order_side=OrderSide.SELL,
                        quantity=qty,
                    )
                    self.submit_order(order)
                    self.partial_taken[iid] = True

                    stop = self.stop_orders.get(iid)
                    if stop is not None:
                        be_price = instrument.make_price(self.entry_price[iid])
                        self.modify_order(stop, trigger_price=be_price)

        # Time stop
        if self.config.max_hold_bars > 0 and count >= self.config.max_hold_bars:
            self._close_instrument(iid)
            return

        # Trailing SMA exit
        sma_key = "sma10" if self.config.trail_period == 10 else "sma20"
        if idx < len(ind[sma_key]):
            sma_val = ind[sma_key][idx]
            if not math.isnan(sma_val) and sma_val > 0 and close < sma_val:
                self._close_instrument(iid)

    def _close_instrument(self, iid: InstrumentId) -> None:
        self.close_all_positions(iid)
        stop = self.stop_orders.get(iid)
        if stop is not None:
            self.cancel_order(stop)
            self.stop_orders[iid] = None
        self._clear_tracking(iid)

    def _clear_tracking(self, iid: InstrumentId) -> None:
        self.entry_bar_count.pop(iid, None)
        self.entry_price.pop(iid, None)
        self.stop_orders.pop(iid, None)
        self.partial_taken.pop(iid, None)

    # -- entry evaluation ----------------------------------------------------

    def _evaluate_entry(
        self, bar: Bar, iid: InstrumentId, idx: int, ind: dict[str, np.ndarray],
    ) -> None:
        close = bar.close.as_double()
        bpd = self.config.bars_per_day

        # Read all indicator values for this bar
        def _v(name: str) -> float:
            arr = ind.get(name)
            if arr is None or idx >= len(arr):
                return float("nan")
            return float(arr[idx])

        sma10 = _v("sma10")
        atr14 = _v("atr14")
        vol_sma20 = _v("vol_sma20")
        rolling_high = _v("rolling_high")
        ret_63 = _v(f"ret_{63 * bpd}")

        vcp_nc = _v("vcp_nc")
        vcp_lcp = _v("vcp_lcp")
        vcp_tr = _v("vcp_tr")
        vcp_vt = _v("vcp_vt")

        flag_pp = _v("flag_pp")
        flag_rp = _v("flag_rp")
        flag_fd = _v("flag_fd")
        flag_vr = _v("flag_vr")

        # Check for NaN in critical indicators
        if math.isnan(ret_63) or math.isnan(atr14) or math.isnan(vol_sma20):
            return

        # Relative strength percentile (configurable lookback, scaled by bars_per_day)
        rs_key = f"ret_{self.config.rs_lookback * bpd}"
        my_ret_rs = _v(rs_key)
        if math.isnan(my_ret_rs):
            return

        all_ret_rs: list[float] = []
        for other_iid, other_ticker in self._ticker_map.items():
            other_ind = self._indicators.get(other_ticker)
            if other_ind is None:
                continue
            other_idx = self._bar_idx.get(other_iid, -1)
            rs_arr = other_ind.get(rs_key)
            if rs_arr is None or other_idx < 0 or other_idx >= len(rs_arr):
                continue
            val = float(rs_arr[other_idx])
            if not math.isnan(val):
                all_ret_rs.append(val)

        if not all_ret_rs:
            return
        rank = sum(1 for v in all_ret_rs if v <= my_ret_rs) / len(all_ret_rs)
        if rank < (1.0 - self.config.rs_pct):
            return

        # Distance from 52-week high
        if not math.isnan(rolling_high) and rolling_high > 0:
            if (rolling_high - close) / rolling_high > self.config.max_dist_52w:
                return

        # Prior move (63-bar return)
        if ret_63 < self.config.min_prior_move:
            return

        # ADR check
        if close > 0 and atr14 / close < self.config.min_adr_pct:
            return

        # Pattern detection (VCP or bull flag)
        vcp_signal = (
            vcp_nc >= 2
            and vcp_lcp < self.config.max_range_pct
            and vcp_tr < 1.0
            and vcp_vt < 1.0
        )

        flag_signal = (
            flag_pp >= self.config.flag_min_pole
            and 0.0 <= flag_rp < self.config.flag_max_retrace
            and self.config.flag_min_days * bpd <= flag_fd <= self.config.flag_max_days * bpd
            and flag_vr < 1.0
        )

        if not (vcp_signal or flag_signal):
            return

        # Volume spike
        vol = bar.volume.as_double()
        if vol_sma20 > 0 and vol < self.config.vol_spike * vol_sma20:
            return

        # Position sizing
        account = self.portfolio.account(self._venue)
        equity = float(account.balance_total(self._quote_currency))

        stop_price = max(bar.low.as_double(), close - atr14)
        stop_dist = abs(close - stop_price)
        if stop_dist <= 0:
            return

        risk_shares = (equity * self.config.risk_pct) / stop_dist
        max_shares = (equity * self.config.max_pos_pct) / close

        adv = vol_sma20 * close
        liq_cap = (self.config.liq_cap_pct * adv) / close if adv > 0 else risk_shares

        shares = min(risk_shares, max_shares, liq_cap)
        if shares < 1:
            return

        instrument = self.instruments[iid]
        qty = instrument.make_qty(Decimal(str(int(shares))))
        if float(qty) <= 0:
            return

        # Submit entry + stop orders
        entry_order = self.order_factory.market(
            instrument_id=iid,
            order_side=OrderSide.BUY,
            quantity=qty,
            time_in_force=TimeInForce.IOC,
        )
        self.submit_order(entry_order)

        stop_order = self.order_factory.stop_market(
            instrument_id=iid,
            order_side=OrderSide.SELL,
            quantity=qty,
            trigger_price=instrument.make_price(stop_price),
            time_in_force=TimeInForce.GTC,
            reduce_only=True,
        )
        self.submit_order(stop_order)

        self.entry_bar_count[iid] = 0
        self.entry_price[iid] = close
        self.stop_orders[iid] = stop_order
        self.partial_taken[iid] = False

    # -- events --------------------------------------------------------------

    def on_event(self, event) -> None:
        iid: InstrumentId | None = getattr(event, "instrument_id", None)
        if iid is None or iid not in self.entry_bar_count:
            return

        from nautilus_trader.model.events import OrderFilled, PositionClosed

        if isinstance(event, PositionClosed):
            self._clear_tracking(iid)
        elif isinstance(event, OrderFilled):
            stop = self.stop_orders.get(iid)
            if stop is not None and event.client_order_id == stop.client_order_id:
                self._clear_tracking(iid)

    # -- shutdown / reset ----------------------------------------------------

    def on_stop(self) -> None:
        for iid in list(self.instruments):
            self.cancel_all_orders(iid)
            self.close_all_positions(iid)

    def on_reset(self) -> None:
        self._bar_idx = {iid: -1 for iid in self._bar_idx}
        self.entry_bar_count.clear()
        self.entry_price.clear()
        self.stop_orders.clear()
        self.partial_taken.clear()
        self.regime_ok = False
