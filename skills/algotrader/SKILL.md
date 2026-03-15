---
name: algotrader
description: >
  Qullamaggie-style swing trading backtester and paper portfolio game. Use when
  asked about: swing trading setups, breakout patterns, VCP, episodic pivots,
  parabolic shorts, backtesting results, strategy analysis, or simulated trading.
  Runs vectorbt backtests on 27 years of US equity daily data (11,922 tickers).
metadata: { "openclaw": { "emoji": "📈", "requires": { "bins": ["uv"] } } }
---

# Swingtrader

Qullamaggie-style swing trading backtester. Four setups (breakout, EP, parabolic, signal_breakout), 11k+ tickers, 27 years of history. Rust engine for fast parameter search; NautilusTrader for production backtesting.

## When to use

- "backtest" / "swing trade" / "breakout" / "VCP" / "flag pattern"
- "episodic pivot" / "EP setup" / "gap and go"
- "parabolic short" / "mean reversion short"
- "Qullamaggie" / "Kristjan" / "momentum setup"
- "how would X strategy have performed"
- "show me winning setups" / "best trades"
- "portfolio" / "my positions" / "P&L"

## Setups

Three independent strategies:

1. **Continuation Breakout** — long. VCP/flag base + RS leader + breakout on volume. Rules: `../stock-trading/references/qullamaggie-breakout.md`
2. **Episodic Pivot (EP)** — long. Gap ≥ 10% on catalyst + massive volume. Rules: `../stock-trading/references/qullamaggie-episodic-pivot.md`
3. **Parabolic Short** — short. Overextended run + first red day = mean reversion. Rules: `../stock-trading/references/qullamaggie-parabolic-short.md`

## Data

Precomputed parquet files in `{baseDir}/data/`:

- `ohlcv.parquet` — all tickers, all dates, wide format (field × ticker MultiIndex)
- `etf_tickers.txt` — ETF symbols to exclude

**Data must be prepared before first use.** See "Data Prep" below.

## Running a backtest

All setups, full history:

```bash
uv run {baseDir}/scripts/backtest.py \
  --data-dir {baseDir}/data \
  --output /tmp/backtest-results.json
```

Single setup with date range:

```bash
uv run {baseDir}/scripts/backtest.py \
  --data-dir {baseDir}/data \
  --setup breakout \
  --start 2020-01-01 \
  --end 2024-12-31 \
  --output /tmp/backtest-results.json
```

Options: `--setup {all,breakout,ep,parabolic}`, `--start YYYY-MM-DD`, `--end YYYY-MM-DD`, `--init-cash 100000`

## Reading results

The output JSON contains:

- `total_return`, `cagr`, `max_drawdown`, `sharpe`, `sortino`
- `win_rate`, `avg_win`, `avg_loss`, `profit_factor`, `avg_hold_days`
- `per_setup` — breakdown by strategy with trades, win rate, total P&L
- `best_trades` / `worst_trades` — top/bottom 10
- `monthly_returns` — monthly P&L percentages
- `equity_curve` — date + equity pairs for charting

Interpret results:

- Expect win rate ~25–35% (Qullamaggie's historical range)
- Profit factor > 1.5 is healthy; > 2.0 is excellent
- Max drawdown > 40% is concerning for a swing strategy

## Data Prep (one-time setup)

Before first use, convert Stooq CSVs to parquet:

```bash
# 1. Unzip Stooq daily data
mkdir -p /tmp/stooq_daily
cd /tmp/stooq_daily
unzip ~/Downloads/d_us_txt.zip

# 2. Run the conversion (one-time, ~5-10 min)
# Convert all CSV files to a single wide parquet + etf_tickers.txt
python3 - <<'EOF'
import pandas as pd
import numpy as np
from pathlib import Path

stooq_root = Path("/tmp/stooq_daily/data/daily/us")
out_dir = Path("{baseDir}/data")
out_dir.mkdir(exist_ok=True)

stock_dirs = [d for d in stooq_root.iterdir() if "stocks" in d.name]
etf_dirs = [d for d in stooq_root.iterdir() if "etf" in d.name]

# Collect ETF tickers
etf_tickers = set()
for etf_dir in etf_dirs:
    for f in etf_dir.rglob("*.us.txt"):
        etf_tickers.add(f.stem.upper().replace(".US", ""))
(out_dir / "etf_tickers.txt").write_text("\n".join(sorted(etf_tickers)))
print(f"ETF tickers: {len(etf_tickers)}")

# Load and pivot all stock CSVs
frames = {}
cols = ["ticker","per","date","time","open","high","low","close","volume","openint"]
for stock_dir in stock_dirs:
    for f in stock_dir.rglob("*.us.txt"):
        ticker = f.stem.upper().replace(".US", "")
        try:
            df = pd.read_csv(f, names=cols, skiprows=1,
                             dtype={"date": str, "time": str,
                                    "open": "float32", "high": "float32",
                                    "low": "float32", "close": "float32",
                                    "volume": "float32"})
            df["date"] = pd.to_datetime(df["date"], format="%Y%m%d")
            df = df.set_index("date").sort_index()
            frames[ticker] = df[["open","high","low","close","volume"]]
        except Exception as e:
            print(f"  skip {ticker}: {e}")

print(f"Loaded {len(frames)} tickers")

# Build unified date index
all_dates = sorted(set().union(*[set(df.index) for df in frames.values()]))
date_idx = pd.DatetimeIndex(all_dates)

# Pivot to wide format with MultiIndex columns (field, ticker)
fields = ["open", "high", "low", "close", "volume"]
wide = {}
for field in fields:
    field_df = pd.DataFrame(
        {ticker: df[field].reindex(date_idx) for ticker, df in frames.items()},
        dtype=np.float32
    )
    wide[field] = field_df

combined = pd.concat(wide, axis=1)
combined.to_parquet(out_dir / "ohlcv.parquet", compression="zstd")
print(f"Saved ohlcv.parquet: {combined.shape}")
EOF
```

## Rules reference

Overview and shared framework: `../stock-trading/references/qullamaggie-rules.md`
Detailed setup rules:

- `../stock-trading/references/qullamaggie-breakout.md`
- `../stock-trading/references/qullamaggie-episodic-pivot.md`
- `../stock-trading/references/qullamaggie-parabolic-short.md`
