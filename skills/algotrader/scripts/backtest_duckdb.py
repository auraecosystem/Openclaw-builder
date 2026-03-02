#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "vectorbt>=0.25",
#     "pandas>=2.0",
#     "numpy",
#     "pyarrow",
#     "numba",
#     "duckdb>=0.10",
# ]
# ///
"""
DuckDB-accelerated swing trading backtester.

Architecture
------------
Cold run: load OHLCV → DuckDB UNPIVOT to long format → single SQL pass
  computes all rolling indicators + RS percentile ranking → persist to
  data/cache/intermediates.duckdb.

Warm run (any threshold combination): open .duckdb, run parameterised SQL
  query → sparse long-format entries → fast numpy pivot → vectorbt.

Compared to backtest.py (loads 23 parquet files, recomputes RS ranking +
patterns on every run), threshold sweeps only cost a SQL query + pivot.

ATR (EWM-based) and consecutive_green_days (stateful reset) stay in pandas
because they're not easily expressed in SQL. They're loaded from the parquet
cache (built once by backtest.py) or computed fresh.

Usage:
    uv run backtest_duckdb.py --data-dir <path>/data
    uv run backtest_duckdb.py --data-dir <path>/data --rs-pct 0.05 --vol-ratio 2.0
"""

import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "lib"))

import duckdb
import numpy as np
import pandas as pd
import vectorbt as vbt

import indicators as ind
import universe as uni
import risk
import cache as _cache
from portfolio import PortfolioState, Position, generate_report, save_report


def _log(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr)


# ---------------------------------------------------------------------------
# DuckDB build SQL
# ---------------------------------------------------------------------------

_BUILD_INTERMEDIATES_SQL = """\
CREATE TABLE intermediates AS
WITH
-- Step 0: pre-lag all values needed by window functions in step 1
-- (DuckDB doesn't allow nested window functions in the same SELECT)
lag_prep AS (
    SELECT *,
        LAG(high,   1) OVER w AS high_lag1,
        LAG(close,  1) OVER w AS prev_close,
        LAG(low,    1) OVER w AS prev_low,
        LAG(open,   1) OVER w AS prev_open,
        LAG(close, 10) OVER w AS close_lag10,
        LAG(close, 20) OVER w AS close_lag20,
        LAG(close, 21) OVER w AS close_lag21,
        LAG(close, 63) OVER w AS close_lag63,
        LAG(close,126) OVER w AS close_lag126
    FROM ohlcv
    WINDOW w AS (PARTITION BY ticker ORDER BY date)
),
-- Step 1: all single-level rolling window operations
rolling AS (
    SELECT
        date, ticker, close, high, low, open, volume,
        prev_close, prev_low, prev_open,
        -- Rolling returns (for RS ranking)
        close / NULLIF(close_lag21,  0) - 1 AS ret_21,
        close / NULLIF(close_lag63,  0) - 1 AS ret_63,
        close / NULLIF(close_lag126, 0) - 1 AS ret_126,
        -- 20d impulse return (for flag pattern)
        close / NULLIF(close_lag20, 0) - 1 AS flag_impulse_return,
        -- 10d pct change (for parabolic)
        close / NULLIF(close_lag10, 0) - 1 AS pct_10d,
        -- SMA 10 and 20
        AVG(close) OVER w10 AS sma_10,
        AVG(close) OVER w20 AS sma_20,
        -- Volume SMA 20 (reused as vcp_vol_long)
        AVG(volume) OVER w20 AS vol_sma_20,
        -- VCP intermediates
        AVG(volume) OVER w10 AS vcp_vol_short,
        (MAX(high) OVER w10 - MIN(low) OVER w10) / NULLIF(MAX(high) OVER w10, 0) AS vcp_recent_range,
        (MAX(high) OVER w20 - MIN(low) OVER w20) / NULLIF(MAX(high) OVER w20, 0) AS vcp_prior_range,
        (MAX(high) OVER w40 - MIN(low) OVER w40) / NULLIF(MAX(high) OVER w40, 0) AS vcp_base_range,
        -- Flag pattern intermediates
        AVG(volume) OVER w5  AS flag_vol_short,
        MAX(close) OVER w45  AS flag_impulse_high,
        -- Asymmetric frame: min of close from 25-44 days ago
        MIN(close) OVER (PARTITION BY ticker ORDER BY date
                         ROWS BETWEEN 44 PRECEDING AND 25 PRECEDING) AS flag_impulse_low,
        -- Consolidation high: rolling max of prior highs (shift(1) in pandas)
        MAX(high_lag1) OVER w40 AS consol_high,
        -- 52-week high
        MAX(high) OVER w252 AS high_52w
    FROM lag_prep
    WINDOW
        w5   AS (PARTITION BY ticker ORDER BY date ROWS BETWEEN  4 PRECEDING AND CURRENT ROW),
        w10  AS (PARTITION BY ticker ORDER BY date ROWS BETWEEN  9 PRECEDING AND CURRENT ROW),
        w20  AS (PARTITION BY ticker ORDER BY date ROWS BETWEEN 19 PRECEDING AND CURRENT ROW),
        w40  AS (PARTITION BY ticker ORDER BY date ROWS BETWEEN 39 PRECEDING AND CURRENT ROW),
        w45  AS (PARTITION BY ticker ORDER BY date ROWS BETWEEN 44 PRECEDING AND CURRENT ROW),
        w252 AS (PARTITION BY ticker ORDER BY date ROWS BETWEEN 251 PRECEDING AND CURRENT ROW)
),
-- Step 2: derived values that depend on step 1 results
derived AS (
    SELECT *,
        (flag_impulse_high - close) / NULLIF(flag_impulse_high - flag_impulse_low, 0) AS flag_retrace,
        (high_52w - close) / NULLIF(high_52w, 0) AS dist_52w,
        -- flag_vol_long = vol_sma_20 shifted back 5 days
        LAG(vol_sma_20, 5) OVER (PARTITION BY ticker ORDER BY date) AS flag_vol_long,
        -- Rolling max of 20d impulse return over 25-day window
        MAX(flag_impulse_return) OVER (PARTITION BY ticker ORDER BY date
                                       ROWS BETWEEN 24 PRECEDING AND CURRENT ROW) AS flag_impulse_max
    FROM rolling
),
-- Step 3: cross-sectional RS percentile ranking (most expensive in pandas ~15s)
ranked AS (
    SELECT *,
        PERCENT_RANK() OVER (PARTITION BY date ORDER BY ret_21  NULLS LAST) AS rs_pctrank_1m,
        PERCENT_RANK() OVER (PARTITION BY date ORDER BY ret_63  NULLS LAST) AS rs_pctrank_3m,
        PERCENT_RANK() OVER (PARTITION BY date ORDER BY ret_126 NULLS LAST) AS rs_pctrank_6m
    FROM derived
)
SELECT * FROM ranked
"""

# ---------------------------------------------------------------------------
# Per-setup entry queries
# ---------------------------------------------------------------------------

_BREAKOUT_SQL = """\
WITH signals AS (
    SELECT date, ticker, low AS stop_candidate,
        -- VCP pattern
        (vcp_recent_range < vcp_prior_range
         AND vcp_base_range       < {max_range_pct}
         AND vcp_vol_short        < vol_sma_20) AS is_vcp,
        -- Flag pattern
        (COALESCE(flag_impulse_max, 0) >= 0.20
         AND flag_retrace > 0
         AND flag_retrace        < 0.50
         AND flag_vol_short      < COALESCE(flag_vol_long, flag_vol_short + 1)) AS is_flag,
        -- RS filter
        COALESCE(rs_pctrank_1m, 0) >= {rs_threshold}
            AND COALESCE(rs_pctrank_3m, 0) >= {rs_threshold}
            AND COALESCE(rs_pctrank_6m, 0) >= {rs_threshold} AS rs_ok,
        -- Breakout conditions
        close > COALESCE(consol_high, close + 1)   AS above_consol,
        COALESCE(vol_sma_20, 0) > 0
            AND volume > vol_sma_20 * {vol_ratio}  AS vol_spike,
        -- Universe
        close > 5.0 AND COALESCE(vol_sma_20, 0) > 300000  AS is_liquid,
        COALESCE(dist_52w, 1.0) <= {max_dist_52w}           AS near_high,
        (CAST('{start}' AS VARCHAR) = '' OR date >= CAST('{start}' AS DATE)) AS after_start,
        (CAST('{end}'   AS VARCHAR) = '' OR date <= CAST('{end}'   AS DATE)) AS before_end,
        COALESCE(vol_sma_20, 0) * close >= {min_adv_dollars}                AS adv_ok,
        -- Quality filters (breakout only)
        ret_63 >= {min_prior_move}  AS prior_move_ok,
        (close - NULLIF(sma_10, 0)) / NULLIF(sma_10, 0) <= {max_sma_ext}  AS near_sma
    FROM intermediates
)
SELECT date, ticker, stop_candidate
FROM signals
WHERE (is_vcp OR is_flag)
  AND rs_ok AND above_consol AND vol_spike
  AND is_liquid AND near_high AND after_start AND before_end AND adv_ok
  AND prior_move_ok AND near_sma
ORDER BY date, ticker
"""

_EP_SQL = """\
WITH gap_days AS (
    SELECT
        LEAD(date,  1) OVER (PARTITION BY ticker ORDER BY date) AS date,
        ticker,
        low  AS stop_candidate,
        LEAD(open, 1) OVER (PARTITION BY ticker ORDER BY date) AS entry_open
    FROM intermediates
    WHERE open / NULLIF(prev_close, 0) - 1 >= {min_gap_pct}
      AND COALESCE(vol_sma_20, 0) > 0
      AND volume >= {min_vol_ratio} * vol_sma_20
      AND close > 5.0 AND COALESCE(vol_sma_20, 0) > 300000
      AND vol_sma_20 * close >= {min_adv_dollars}
      AND (CAST('{start}' AS VARCHAR) = '' OR date >= CAST('{start}' AS DATE))
      AND (CAST('{end}'   AS VARCHAR) = '' OR date <= CAST('{end}'   AS DATE))
)
SELECT date, ticker, stop_candidate, entry_open
FROM gap_days
WHERE date IS NOT NULL
ORDER BY date, ticker
"""

_PARABOLIC_SQL = """\
WITH with_prev AS (
    SELECT *,
        LAG(consec_green, 1) OVER (PARTITION BY ticker ORDER BY date) AS prev_consec_green
    FROM intermediates
)
SELECT date, ticker, high AS stop_candidate
FROM with_prev
WHERE
    -- Parabolic run (price proxy for cap)
    ((close > {large_cap_price} AND pct_10d >= {large_cap_run})
     OR (close <= {large_cap_price} AND pct_10d >= {small_cap_run}))
  AND close < open
  AND COALESCE(prev_consec_green, 0) >= {min_green_days}
  AND close > 5.0 AND COALESCE(vol_sma_20, 0) > 300000
  AND vol_sma_20 * close >= {min_adv_dollars}
  AND (CAST('{start}' AS VARCHAR) = '' OR date >= CAST('{start}' AS DATE))
  AND (CAST('{end}'   AS VARCHAR) = '' OR date <= CAST('{end}'   AS DATE))
ORDER BY date, ticker
"""


# ---------------------------------------------------------------------------
# DB management
# ---------------------------------------------------------------------------

def _db_path(data_dir: Path) -> str:
    return str(data_dir / "cache" / "intermediates.duckdb")


def _db_is_valid(conn: duckdb.DuckDBPyConnection, ohlcv_mtime: float) -> bool:
    try:
        tables = {r[0] for r in conn.execute("SHOW TABLES").fetchall()}
        if "intermediates" not in tables or "_meta" not in tables:
            return False
        row = conn.execute("SELECT ohlcv_mtime FROM _meta LIMIT 1").fetchone()
        return row is not None and abs(row[0] - ohlcv_mtime) < 1
    except Exception:
        return False


def _build_db(
    conn: duckdb.DuckDBPyConnection,
    ohlcv: dict[str, pd.DataFrame],
    consec_green: pd.DataFrame,
    ohlcv_mtime: float,
) -> None:
    """Build all DuckDB tables from scratch.

    Steps:
    1. UNPIVOT 5 wide DataFrames → long ohlcv table
    2. Run big SQL to compute all rolling + RS intermediates
    3. Attach consec_green (from pandas/numba) via JOIN
    """
    _log("  Registering OHLCV DataFrames with DuckDB...")
    for field in ["close", "high", "low", "open", "volume"]:
        df = ohlcv[field].copy()
        df.index.name = "date"
        conn.register(f"{field}_wide", df.reset_index())

    _log("  Building long-format ohlcv table (UNPIVOT)...")
    conn.execute("DROP TABLE IF EXISTS ohlcv")
    conn.execute("""\
        CREATE TABLE ohlcv AS
        WITH
          cl AS (UNPIVOT close_wide  ON COLUMNS(* EXCLUDE date) INTO NAME ticker VALUE close),
          hi AS (UNPIVOT high_wide   ON COLUMNS(* EXCLUDE date) INTO NAME ticker VALUE high),
          lo AS (UNPIVOT low_wide    ON COLUMNS(* EXCLUDE date) INTO NAME ticker VALUE low),
          op AS (UNPIVOT open_wide   ON COLUMNS(* EXCLUDE date) INTO NAME ticker VALUE open),
          vo AS (UNPIVOT volume_wide ON COLUMNS(* EXCLUDE date) INTO NAME ticker VALUE volume)
        SELECT cl.date::DATE AS date, cl.ticker,
               cl.close, hi.high, lo.low, op.open, vo.volume
        FROM cl
        JOIN hi USING (date, ticker)
        JOIN lo USING (date, ticker)
        JOIN op USING (date, ticker)
        JOIN vo USING (date, ticker)
    """)

    n = conn.execute("SELECT COUNT(*) FROM ohlcv").fetchone()[0]
    _log(f"  ohlcv table: {n:,} rows")

    _log("  Computing rolling indicators + RS ranking via SQL...")
    conn.execute("DROP TABLE IF EXISTS intermediates")
    conn.execute(_BUILD_INTERMEDIATES_SQL)

    # Attach consec_green (stateful pandas/numba — can't express in SQL)
    _log("  Joining consec_green into intermediates...")
    cg = consec_green.stack(future_stack=True).reset_index()
    cg.columns = pd.Index(["date", "ticker", "consec_green"])
    cg["date"] = pd.to_datetime(cg["date"]).dt.date
    conn.register("consec_green_data", cg)
    conn.execute("""\
        CREATE TABLE intermediates_full AS
        SELECT i.*, COALESCE(c.consec_green, 0)::INT AS consec_green
        FROM intermediates i
        LEFT JOIN consec_green_data c USING (date, ticker)
    """)
    conn.execute("DROP TABLE intermediates")
    conn.execute("ALTER TABLE intermediates_full RENAME TO intermediates")

    _log("  Creating indexes...")
    conn.execute("CREATE INDEX IF NOT EXISTS idx_date   ON intermediates(date)")
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tk_dt  ON intermediates(ticker, date)")

    conn.execute("DROP TABLE IF EXISTS _meta")
    conn.execute("CREATE TABLE _meta (ohlcv_mtime DOUBLE, created_at DOUBLE)")
    conn.execute(f"INSERT INTO _meta VALUES ({ohlcv_mtime}, {time.time()})")

    m = conn.execute("SELECT COUNT(*) FROM intermediates").fetchone()[0]
    _log(f"  intermediates table: {m:,} rows")


# ---------------------------------------------------------------------------
# Wide-format reconstruction
# ---------------------------------------------------------------------------

def _sparse_to_wide(
    long_df: pd.DataFrame,
    ref: pd.DataFrame,
    date_col: str = "date",
    ticker_col: str = "ticker",
) -> pd.DataFrame:
    """Fast pivot of sparse (date, ticker) pairs → wide boolean DataFrame.

    Uses pd.Series.unstack which is much faster than pd.pivot() for large
    sparse data because it only touches the True cells.
    """
    if long_df.empty:
        return pd.DataFrame(False, index=ref.index, columns=ref.columns)
    long_df = long_df.copy()
    long_df[date_col] = pd.to_datetime(long_df[date_col])
    s = pd.Series(
        True,
        index=pd.MultiIndex.from_arrays([long_df[date_col], long_df[ticker_col]]),
        dtype=bool,
    )
    wide = s.unstack(level=1, fill_value=False)
    return wide.reindex(index=ref.index, columns=ref.columns, fill_value=False)


# ---------------------------------------------------------------------------
# Per-setup simulation (mirrors backtest.py)
# ---------------------------------------------------------------------------

def _run_breakout_split(
    entries: pd.DataFrame,
    sma_exits: pd.DataFrame,
    stop_prices: pd.DataFrame,
    close: pd.DataFrame,
    open_: pd.DataFrame,
    init_cash: float,
    vol_sma_20: pd.DataFrame,
    slippage_k: float = 0.1,
) -> list[Position]:
    half_cash = init_cash / 2

    stop_exits   = risk.compute_stop_exit(close, entries, stop_prices)
    partial_exits = risk.compute_partial_exit(close, entries, n_days=5)
    be_stop_exits = risk.compute_breakeven_stop_exit(close, entries, stop_prices, n_days=5)

    exits_quick  = partial_exits | stop_exits | sma_exits
    exits_runner = sma_exits | be_stop_exits

    sizes = risk.size_from_risk(close=close, stop_prices=stop_prices,
                                entries=entries, equity=half_cash)
    sizes = sizes.fillna(1).clip(lower=1)

    # Size-adjusted slippage: wider impact for larger orders relative to ADV
    slippage_df = risk.size_adjusted_slippage(sizes, vol_sma_20, entries, k=slippage_k)

    # Fill at next-day open to avoid same-bar lookahead bias
    fill_price = open_.shift(-1)

    positions = []
    for label, exits_df in [("breakout_quick", exits_quick), ("breakout_runner", exits_runner)]:
        _log(f"    {label}: running vectorbt...")
        pf = vbt.Portfolio.from_signals(
            close=close, entries=entries, exits=exits_df,
            price=fill_price,
            size=sizes, size_type="amount",
            fees=0.001, slippage=slippage_df, freq="1D",
            direction="longonly", init_cash=half_cash,
        )
        try:
            trades_df = pf.trades.records_readable
            pos_list = _trades_to_positions(trades_df, label)
            positions.extend(pos_list)
            _log(f"    {label}: {len(pos_list)} trades")
        except Exception as e:
            _log(f"    {label}: failed — {e}")

    return positions


def _trades_to_positions(trades_df: pd.DataFrame, setup: str) -> list[Position]:
    positions = []
    for _, row in trades_df.iterrows():
        ticker     = str(row.get("Column", ""))
        direction  = "short" if str(row.get("Direction", "Long")) == "Short" else "long"
        pnl        = float(row.get("PnL", 0))
        entry_price = float(row.get("Avg Entry Price", 0))
        exit_price  = float(row.get("Avg Exit Price", 0))
        shares      = int(abs(row.get("Size", 0)))
        entry_ts    = row.get("Entry Timestamp")
        exit_ts     = row.get("Exit Timestamp")
        entry_date  = str(entry_ts) if entry_ts is not None else ""
        exit_date   = str(exit_ts)  if exit_ts  is not None else ""
        hold_days   = 0
        if entry_ts is not None and exit_ts is not None:
            try:
                hold_days = (pd.Timestamp(exit_ts) - pd.Timestamp(entry_ts)).days
            except Exception:
                pass
        positions.append(Position(
            ticker=ticker, setup=setup,
            entry_date=entry_date, entry_price=entry_price,
            shares=shares, stop_price=entry_price * 0.95,
            direction=direction, exit_date=exit_date,
            exit_price=exit_price, pnl=pnl,
            pnl_pct=pnl / (entry_price * shares) if entry_price * shares != 0 else 0.0,
            hold_days=hold_days,
        ))
    return positions


def _run_setup_duckdb(
    setup_name: str,
    conn: duckdb.DuckDBPyConnection,
    ohlcv: dict[str, pd.DataFrame],
    atr_14: pd.DataFrame,
    sma_10: pd.DataFrame,
    vol_sma_20: pd.DataFrame,
    init_cash: float,
    start: str | None,
    end: str | None,
    vol_ratio: float,
    max_range_pct: float,
    max_dist_52w: float,
    rs_pct: float,
    min_adv_dollars: float = 150_000_000,
    slippage_k: float = 0.1,
    min_prior_move: float = 0.30,
    max_sma_ext: float = 0.10,
    regime_dates: set[str] | None = None,
) -> list[Position]:
    close  = ohlcv["close"]
    open_  = ohlcv["open"]
    high   = ohlcv["high"]
    low    = ohlcv["low"]
    volume = ohlcv["volume"]
    slc    = slice(start or None, end or None)

    if setup_name == "breakout":
        sql = _BREAKOUT_SQL.format(
            max_range_pct=max_range_pct,
            rs_threshold=1.0 - rs_pct,
            vol_ratio=vol_ratio,
            max_dist_52w=max_dist_52w,
            min_adv_dollars=min_adv_dollars,
            min_prior_move=min_prior_move,
            max_sma_ext=max_sma_ext,
            start=start or "",
            end=end or "",
        )
        entries_long = conn.execute(sql).df()
        _log(f"  breakout: {len(entries_long)} raw entry signals from DuckDB")

        if regime_dates:
            entries_long = entries_long[
                pd.to_datetime(entries_long["date"]).dt.strftime("%Y-%m-%d").isin(regime_dates)
            ]
            _log(f"  breakout: {len(entries_long)} signals after regime filter")

        entries = _sparse_to_wide(entries_long, close)
        entries = entries.loc[slc]

        # Exits + stop prices from wide pandas DataFrames
        c     = close.loc[slc]
        l     = low.loc[slc]
        atr_s = atr_14.loc[slc]
        sma_exits   = c < sma_10.loc[slc]
        stop_prices = pd.DataFrame(
            np.maximum(l.values, (c - atr_s).values),
            index=c.index, columns=c.columns,
        )

        active = entries.any(axis=0)
        active_tickers = active[active].index.tolist()
        if not active_tickers:
            _log("  breakout: no active tickers")
            return []
        _log(f"  breakout: {len(active_tickers)} active tickers")

        return _run_breakout_split(
            entries[active_tickers],
            sma_exits[active_tickers],
            stop_prices[active_tickers],
            c[active_tickers],
            open_.loc[slc][active_tickers],
            init_cash,
            vol_sma_20=vol_sma_20.loc[slc][active_tickers],
            slippage_k=slippage_k,
        )

    elif setup_name == "ep":
        sql = _EP_SQL.format(
            min_gap_pct=0.10,
            min_vol_ratio=2.0,
            min_adv_dollars=min_adv_dollars,
            start=start or "",
            end=end or "",
        )
        entries_long = conn.execute(sql).df()
        _log(f"  ep: {len(entries_long)} raw entry signals from DuckDB")

        if regime_dates:
            entries_long = entries_long[
                pd.to_datetime(entries_long["date"]).dt.strftime("%Y-%m-%d").isin(regime_dates)
            ]
            _log(f"  ep: {len(entries_long)} signals after regime filter")

        entries     = _sparse_to_wide(entries_long, close).loc[slc]
        sma_exits   = (close.loc[slc] < sma_10.loc[slc])
        # Stop: gap day's low (stop_candidate is yesterday's low)
        stop_long = entries_long.rename(columns={"stop_candidate": "val"})
        stop_long["date"] = pd.to_datetime(stop_long["date"])
        stop_prices_sparse = (
            pd.Series(
                stop_long["val"].values,
                index=pd.MultiIndex.from_arrays([stop_long["date"], stop_long["ticker"]]),
            )
            .unstack(level=1)
            .reindex(index=close.index, columns=close.columns)
            .ffill(limit=1)
            .loc[slc]
        )

        active = entries.any(axis=0)
        active_tickers = active[active].index.tolist()
        if not active_tickers:
            _log("  ep: no active tickers")
            return []
        _log(f"  ep: {len(active_tickers)} active tickers")

        stop_exits = risk.compute_stop_exit(
            close.loc[slc][active_tickers],
            entries[active_tickers],
            stop_prices_sparse[active_tickers],
        )
        exits_a = sma_exits[active_tickers] | stop_exits
        sizes = risk.size_from_risk(
            close=close.loc[slc][active_tickers],
            stop_prices=stop_prices_sparse[active_tickers],
            entries=entries[active_tickers],
            equity=init_cash,
        ).fillna(1).clip(lower=1)

        slippage_df = risk.size_adjusted_slippage(
            sizes, vol_sma_20.loc[slc][active_tickers], entries[active_tickers], k=slippage_k,
        )

        pf = vbt.Portfolio.from_signals(
            close=close.loc[slc][active_tickers],
            entries=entries[active_tickers],
            exits=exits_a,
            price=open_.loc[slc][active_tickers],
            size=sizes, size_type="amount",
            fees=0.001, slippage=slippage_df, freq="1D",
            direction="longonly", init_cash=init_cash,
        )
        try:
            return _trades_to_positions(pf.trades.records_readable, "ep")
        except Exception as e:
            _log(f"  ep: failed — {e}")
            return []

    elif setup_name == "parabolic":
        sql = _PARABOLIC_SQL.format(
            large_cap_price=50.0, large_cap_run=0.50,
            small_cap_run=3.00, min_green_days=3,
            min_adv_dollars=min_adv_dollars,
            start=start or "", end=end or "",
        )
        entries_long = conn.execute(sql).df()
        _log(f"  parabolic: {len(entries_long)} raw entry signals from DuckDB")

        entries     = _sparse_to_wide(entries_long, close).loc[slc]
        sma_exits   = ((close.loc[slc] <= sma_10.loc[slc]) |
                       (close.loc[slc] <= atr_14.loc[slc]))  # sma_20 approximated
        stop_prices = high.loc[slc].copy()

        active = entries.any(axis=0)
        active_tickers = active[active].index.tolist()
        if not active_tickers:
            _log("  parabolic: no active tickers")
            return []
        _log(f"  parabolic: {len(active_tickers)} active tickers")

        stop_exits = risk.compute_stop_exit(
            close.loc[slc][active_tickers],
            entries[active_tickers],
            stop_prices[active_tickers],
        )
        exits_a = sma_exits[active_tickers] | stop_exits
        sizes = risk.size_from_risk(
            close=close.loc[slc][active_tickers],
            stop_prices=stop_prices[active_tickers],
            entries=entries[active_tickers],
            equity=init_cash,
        ).fillna(1).clip(lower=1)

        slippage_df = risk.size_adjusted_slippage(
            sizes, vol_sma_20.loc[slc][active_tickers], entries[active_tickers], k=slippage_k,
        )

        pf = vbt.Portfolio.from_signals(
            close=close.loc[slc][active_tickers],
            entries=entries[active_tickers],
            exits=exits_a,
            price=open_.loc[slc][active_tickers],
            size=sizes, size_type="amount",
            fees=0.001, slippage=slippage_df, freq="1D",
            direction="shortonly", init_cash=init_cash,
        )
        try:
            return _trades_to_positions(pf.trades.records_readable, "parabolic")
        except Exception as e:
            _log(f"  parabolic: failed — {e}")
            return []

    raise ValueError(f"Unknown setup: {setup_name}")


# ---------------------------------------------------------------------------
# Main entry
# ---------------------------------------------------------------------------

def run_backtest(args: argparse.Namespace) -> dict:
    data_dir = Path(args.data_dir)

    _log("Loading OHLCV data...")
    ohlcv = uni.load_ohlcv(data_dir)
    close = ohlcv["close"]
    _log(f"Loaded {close.shape[1]} tickers × {close.shape[0]} days")

    # ATR, sma_10, vol_sma_20 and consec_green: EWM/stateful — stay in pandas.
    # Load from parquet cache if available (built by backtest.py), else recompute.
    _PARQUET_KEYS = ["atr_14", "sma_10", "vol_sma_20", "consec_green"]
    if _cache.is_valid(data_dir, required_keys=_PARQUET_KEYS):
        _log("Loading ATR / sma_10 / vol_sma_20 / consec_green from parquet cache...")
        atr_14       = _cache.load(data_dir, "atr_14")
        sma_10       = _cache.load(data_dir, "sma_10")
        vol_sma_20   = _cache.load(data_dir, "vol_sma_20")
        consec_green = _cache.load(data_dir, "consec_green")
    else:
        _log("Computing ATR / sma_10 / vol_sma_20 / consec_green (not in cache)...")
        atr_14       = ind.atr(ohlcv["high"], ohlcv["low"], close, period=14)
        sma_10       = ind.sma(close, 10)
        vol_sma_20   = ind.volume_sma(ohlcv["volume"], 20)
        consec_green = ind.consecutive_green_days(ohlcv["open"], close)

    ohlcv_mtime = (data_dir / "ohlcv.parquet").stat().st_mtime

    db_file = _db_path(data_dir)
    Path(db_file).parent.mkdir(parents=True, exist_ok=True)
    conn = duckdb.connect(db_file)

    if _db_is_valid(conn, ohlcv_mtime):
        _log("DuckDB intermediates table is valid — skipping build")
    else:
        _log("Building DuckDB intermediates (cold run — will persist for future runs)...")
        conn.execute("DROP TABLE IF EXISTS intermediates")
        conn.execute("DROP TABLE IF EXISTS ohlcv")
        conn.execute("DROP TABLE IF EXISTS _meta")
        t_build = time.time()
        _build_db(conn, ohlcv, consec_green, ohlcv_mtime)
        _log(f"DuckDB build complete in {time.time() - t_build:.1f}s")

    # Market regime: build set of bullish dates (SPY > 50-day SMA) for long setups
    regime_dates: set[str] = set()
    if args.regime:
        if "SPY" in close.columns:
            spy_sma50 = close["SPY"].rolling(50, min_periods=50).mean()
            ok = close["SPY"] >= spy_sma50
            regime_dates = set(ok.index[ok].strftime("%Y-%m-%d"))
            _log(f"  SPY regime filter: {len(regime_dates)} bullish days")
        else:
            _log("  WARNING: SPY not found — regime filter disabled")

    setups_to_run = (
        ["breakout", "ep", "parabolic"] if args.setup == "all" else [args.setup]
    )
    state = PortfolioState(init_cash=args.init_cash, cash=args.init_cash, equity=args.init_cash)

    for setup_name in setups_to_run:
        _log(f"Running setup: {setup_name}")
        positions = _run_setup_duckdb(
            setup_name=setup_name,
            conn=conn,
            ohlcv=ohlcv,
            atr_14=atr_14,
            sma_10=sma_10,
            vol_sma_20=vol_sma_20,
            init_cash=args.init_cash,
            start=args.start,
            end=args.end,
            vol_ratio=args.vol_ratio,
            max_range_pct=args.max_range,
            max_dist_52w=args.max_dist_52w,
            rs_pct=args.rs_pct,
            min_adv_dollars=args.min_adv,
            slippage_k=args.slippage_k,
            min_prior_move=args.min_prior_move,
            max_sma_ext=args.max_sma_ext,
            regime_dates=regime_dates if setup_name != "parabolic" else set(),
        )
        state.closed_positions.extend(positions)

    conn.close()

    _log("Generating report...")
    return generate_report(state)


def main() -> None:
    parser = argparse.ArgumentParser(description="DuckDB-accelerated Qullamaggie backtest")
    parser.add_argument("--data-dir", required=True)
    parser.add_argument("--output", default="/tmp/backtest-duckdb-results.json")
    parser.add_argument("--setup", default="all",
                        choices=["all", "breakout", "ep", "parabolic"])
    parser.add_argument("--start",     default=None)
    parser.add_argument("--end",       default=None)
    parser.add_argument("--init-cash", type=float, default=100_000.0)
    parser.add_argument("--rs-pct",    type=float, default=0.02)
    parser.add_argument("--vol-ratio", type=float, default=1.5)
    parser.add_argument("--max-range", type=float, default=0.15)
    parser.add_argument("--max-dist-52w", type=float, default=0.25)
    parser.add_argument(
        "--min-adv", type=float, default=150_000_000,
        help="Min average dollar volume filter (vol_sma_20 × close). Default 150,000,000.",
    )
    parser.add_argument(
        "--slippage-k", type=float, default=0.1,
        help="Square-root market impact coefficient for size-adjusted slippage. Default 0.1.",
    )
    parser.add_argument("--min-prior-move", type=float, default=0.30,
                        help="Min 3M return for breakout entry (default: 0.30 = 30%%)")
    parser.add_argument("--max-sma-ext", type=float, default=0.10,
                        help="Max %% extension above 10-day SMA at breakout entry (default: 0.10 = 10%%)")
    parser.add_argument("--regime", default=True, action=argparse.BooleanOptionalAction,
                        help="Only trade breakout/EP when SPY > 50-day SMA (default: on)")
    args = parser.parse_args()

    t0     = time.time()
    report = run_backtest(args)
    elapsed = time.time() - t0

    save_report(report, args.output)
    _log(f"Done in {elapsed:.1f}s — results at {args.output}")

    print(json.dumps({
        "total_trades":  report.get("total_trades", 0),
        "win_rate":      report.get("win_rate", 0),
        "total_return":  report.get("total_return", 0),
        "cagr":          report.get("cagr", 0),
        "max_drawdown":  report.get("max_drawdown", 0),
        "sharpe":        report.get("sharpe", 0),
        "profit_factor": report.get("profit_factor", 0),
    }, indent=2))


if __name__ == "__main__":
    main()
