"""
Entry/exit signal generation for each Qullamaggie setup.

Each function takes wide DataFrames (DatetimeIndex × tickers) and returns a dict
with keys: 'entries', 'exits', 'stop_prices' — all wide DataFrames of the same shape.

These are fed directly into vectorbt's Portfolio.from_signals().
"""

import pandas as pd
import numpy as np


def continuation_breakout_signals(
    close: pd.DataFrame,
    high: pd.DataFrame,
    low: pd.DataFrame,
    volume: pd.DataFrame,
    atr_14: pd.DataFrame,
    sma_10: pd.DataFrame,
    vol_sma_20: pd.DataFrame,
    is_pattern: pd.DataFrame,
    consol_high: pd.DataFrame,
    vol_ratio: float = 1.5,
    min_adv_dollars: float = 1_000_000,
) -> dict[str, pd.DataFrame]:
    """Setup 1: Continuation Breakout (Flag / VCP).

    Entry: pattern present AND close breaks consolidation high AND volume spike.
    Exit: close drops below 10-day SMA (trailing stop).
    Stop: day's low, capped at entry - 1×ATR.

    Partial exits (sell half at day 3-5) are handled in backtest.py by running
    two half-sized portfolios — one with a 5-day forced exit, one trailing SMA.
    """
    # Entry: all conditions simultaneously
    entries = (
        is_pattern
        & (close > consol_high)
        & (volume > vol_ratio * vol_sma_20)
    )

    # Liquidity filter: average dollar volume must meet minimum threshold
    entries = entries & (vol_sma_20 * close >= min_adv_dollars)

    # Trailing stop exit: close falls below 10-day SMA
    exits = close < sma_10

    # Stop price: day's low, but no wider than 1 ATR below close
    # We store this for computing stop exits in risk.py
    stop_prices = pd.DataFrame(
        np.maximum(low.values, (close - atr_14).values),
        index=close.index,
        columns=close.columns,
    )

    return {"entries": entries, "exits": exits, "stop_prices": stop_prices}


def episodic_pivot_signals(
    open_: pd.DataFrame,
    close: pd.DataFrame,
    low: pd.DataFrame,
    volume: pd.DataFrame,
    vol_sma_20: pd.DataFrame,
    sma_10: pd.DataFrame,
    min_gap_pct: float = 0.10,
    min_vol_ratio: float = 2.0,
    min_adv_dollars: float = 1_000_000,
) -> dict[str, pd.DataFrame]:
    """Setup 2: Episodic Pivot (gap-up on catalyst).

    Gap is detected after the daily bar closes, so entry is the NEXT trading day.
    Signal is shifted forward by 1 day to avoid lookahead.
    Entry price in backtest.py uses next day's open (price=open_.shift(-1)).

    Stop: gap day's low (carried forward to entry day via shift).
    Exit: close drops below 10-day SMA.
    """
    prev_close = close.shift(1)
    gap_day = (
        ((open_ / prev_close) - 1 >= min_gap_pct)
        & (volume >= min_vol_ratio * vol_sma_20)
    )

    # Entry is the day AFTER the gap (shift forward 1)
    entries = gap_day.shift(1).fillna(False).astype(bool)

    # Liquidity filter: average dollar volume must meet minimum threshold
    entries = entries & (vol_sma_20 * close >= min_adv_dollars)

    # Trailing stop exit
    exits = close < sma_10

    # Stop price: gap day's low, carried to entry day
    stop_prices = low.shift(1).where(gap_day.shift(1).fillna(False))
    stop_prices = stop_prices.ffill(limit=1)

    return {"entries": entries, "exits": exits, "stop_prices": stop_prices}


def parabolic_short_signals(
    open_: pd.DataFrame,
    close: pd.DataFrame,
    high: pd.DataFrame,
    sma_10: pd.DataFrame,
    sma_20: pd.DataFrame,
    consec_green: pd.DataFrame,
    pct_10d: pd.DataFrame,
    vol_sma_20: pd.DataFrame | None = None,
    large_cap_price: float = 50.0,
    large_cap_run: float = 0.50,
    small_cap_run: float = 3.00,
    min_green_days: int = 3,
    min_adv_dollars: float = 1_000_000,
) -> dict[str, pd.DataFrame]:
    """Setup 3: Parabolic Short (mean reversion).

    Entry: first red day after 3+ green days and a parabolic run.
    Exit: cover when close reaches 10-day or 20-day SMA.
    Stop: entry day's high.
    Direction is 'shortonly' in vectorbt.
    """
    # Parabolic run condition (using close as large-cap proxy)
    is_large = close > large_cap_price
    run_ok = (is_large & (pct_10d >= large_cap_run)) | (~is_large & (pct_10d >= small_cap_run))

    # First red day: prior day had 3+ green days AND today close < open
    prior_green_streak = consec_green.shift(1).fillna(0)
    first_red = (prior_green_streak >= min_green_days) & (close < open_)

    entries = run_ok & first_red

    # Liquidity filter: average dollar volume must meet minimum threshold
    if vol_sma_20 is not None:
        entries = entries & (vol_sma_20 * close >= min_adv_dollars)

    # Cover at either SMA — price touches the mean reversion target
    exits = (close <= sma_10) | (close <= sma_20)

    # Stop: entry day's high (if price reclaims it, the short is wrong)
    stop_prices = high.copy()

    return {"entries": entries, "exits": exits, "stop_prices": stop_prices}
