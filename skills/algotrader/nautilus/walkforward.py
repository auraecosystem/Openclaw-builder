"""Walk-forward analysis for the NautilusTrader Qullamaggie strategy.

Rolls through time windows: optimize on in-sample, validate on out-of-sample.
Computes walk-forward efficiency (WFE = OOS Sharpe / IS Sharpe).

Uses hourly bars resampled from 5-minute data by default.

Usage:
    python -m nautilus.walkforward --data-dir data-crypto
    python -m nautilus.walkforward --data-dir data-crypto --is-months 12 --oos-months 6
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

_project_root = str(Path(__file__).resolve().parent.parent)
if _project_root not in sys.path:
    sys.path.insert(0, _project_root)

from nautilus.evolve import NautilusTransport, DEFAULT_TICKERS
from nautilus.validate import run_validation
from scripts.evolve import evolve


@dataclass
class WindowResult:
    window: int
    is_start: str
    is_end: str
    oos_start: str
    oos_end: str
    is_sharpe: float = 0.0
    oos_sharpe: float = 0.0
    is_pf: float = 0.0
    oos_pf: float = 0.0
    is_trades: int = 0
    oos_trades: int = 0
    is_return: float = 0.0
    oos_return: float = 0.0
    oos_win_rate: float = 0.0
    oos_max_dd: float = 0.0
    best_params: dict = field(default_factory=dict)


def generate_windows(
    first_date: str,
    last_date: str,
    is_months: int,
    oos_months: int,
    step_months: int,
) -> list[tuple[str, str, str, str]]:
    """Generate (is_start, is_end, oos_start, oos_end) date tuples."""
    from datetime import datetime
    from dateutil.relativedelta import relativedelta

    start = datetime.strptime(first_date, "%Y-%m-%d")
    end = datetime.strptime(last_date, "%Y-%m-%d")
    windows = []

    cursor = start
    while True:
        is_start = cursor
        is_end = cursor + relativedelta(months=is_months) - relativedelta(days=1)
        oos_start = is_end + relativedelta(days=1)
        oos_end = oos_start + relativedelta(months=oos_months) - relativedelta(days=1)

        if oos_end > end:
            break

        windows.append((
            is_start.strftime("%Y-%m-%d"),
            is_end.strftime("%Y-%m-%d"),
            oos_start.strftime("%Y-%m-%d"),
            oos_end.strftime("%Y-%m-%d"),
        ))
        cursor += relativedelta(months=step_months)

    return windows


def run_walkforward(
    data_dir: Path,
    tickers: list[str],
    windows: list[tuple[str, str, str, str]],
    generations: int = 15,
    pop_size: int = 30,
    n_workers: int = 8,
    fitness: str = "sharpe",
    bar_spec: str = "1-HOUR-LAST",
    resample: str = "1h",
) -> list[WindowResult]:
    """Run walk-forward analysis across all windows."""
    results = []

    for i, (is_start, is_end, oos_start, oos_end) in enumerate(windows):
        print(f"\n{'='*70}", file=sys.stderr)
        print(f"Window {i+1}/{len(windows)}", file=sys.stderr)
        print(f"  IS:  {is_start} → {is_end}", file=sys.stderr)
        print(f"  OOS: {oos_start} → {oos_end}", file=sys.stderr)
        print(f"{'='*70}", file=sys.stderr)

        # --- In-sample: optimize ---
        print(f"\n--- Optimizing (IS) ---", file=sys.stderr)
        t0 = time.time()

        transport = NautilusTransport(
            data_dir=data_dir,
            tickers=tickers,
            bar_spec=bar_spec,
            timeframe="5m",
            resample=resample,
            start=is_start,
            end=is_end,
            n_workers=n_workers,
        )
        transport.connect()

        try:
            best_params, best_report = evolve(
                transport=transport,
                generations=generations,
                pop_size=pop_size,
                metric=fitness,
                crypto=True,
            )
        finally:
            transport.close()

        is_elapsed = time.time() - t0

        if best_report is None:
            print(f"  No viable params found, skipping OOS", file=sys.stderr)
            results.append(WindowResult(
                window=i+1,
                is_start=is_start, is_end=is_end,
                oos_start=oos_start, oos_end=oos_end,
            ))
            continue

        is_sharpe = best_report.get("sharpe", 0.0)
        is_pf = best_report.get("profit_factor", 0.0)
        is_trades = best_report.get("total_trades", 0)
        is_return = best_report.get("total_return", 0.0)

        print(f"\n  IS result ({is_elapsed:.0f}s):", file=sys.stderr)
        print(f"    Sharpe={is_sharpe:.2f}  PF={is_pf:.2f}  trades={is_trades}  ret={is_return:.2%}", file=sys.stderr)

        # --- Out-of-sample: validate ---
        print(f"\n--- Validating (OOS) ---", file=sys.stderr)
        t1 = time.time()

        oos_report = run_validation(
            data_dir=data_dir,
            tickers=tickers,
            params=best_params,
            resample=resample,
            start=oos_start,
            end=oos_end,
            log_level="ERROR",
        )

        oos_elapsed = time.time() - t1

        oos_sharpe = oos_report.get("sharpe", 0.0)
        oos_pf = oos_report.get("profit_factor", 0.0)
        oos_trades = oos_report.get("total_trades", 0)
        oos_return = oos_report.get("total_return", 0.0)
        oos_wr = oos_report.get("win_rate", 0.0)
        oos_dd = oos_report.get("max_drawdown", 0.0)

        print(f"\n  OOS result ({oos_elapsed:.0f}s):", file=sys.stderr)
        print(f"    Sharpe={oos_sharpe:.2f}  PF={oos_pf:.2f}  trades={oos_trades}  ret={oos_return:.2%}  WR={oos_wr:.1%}  DD={oos_dd:.1%}", file=sys.stderr)

        wfe = oos_sharpe / is_sharpe if is_sharpe > 0 else 0.0
        print(f"    WFE = {wfe:.2f}", file=sys.stderr)

        results.append(WindowResult(
            window=i+1,
            is_start=is_start, is_end=is_end,
            oos_start=oos_start, oos_end=oos_end,
            is_sharpe=is_sharpe, oos_sharpe=oos_sharpe,
            is_pf=is_pf, oos_pf=oos_pf,
            is_trades=is_trades, oos_trades=oos_trades,
            is_return=is_return, oos_return=oos_return,
            oos_win_rate=oos_wr, oos_max_dd=oos_dd,
            best_params=best_params,
        ))

    return results


def print_summary(results: list[WindowResult]) -> None:
    """Print walk-forward summary table."""
    print(f"\n{'='*90}")
    print(f"WALK-FORWARD SUMMARY")
    print(f"{'='*90}")
    print(f"{'Win':>4} {'IS period':>23} {'OOS period':>23}  {'IS Sh':>6} {'OOS Sh':>6} {'WFE':>5}  {'OOS PF':>6} {'OOS ret':>7} {'OOS tr':>6}")
    print(f"{'-'*90}")

    oos_sharpes = []
    wfes = []
    oos_returns = []
    oos_profitable = 0

    for r in results:
        wfe = r.oos_sharpe / r.is_sharpe if r.is_sharpe > 0 else 0.0
        wfes.append(wfe)
        oos_sharpes.append(r.oos_sharpe)
        oos_returns.append(r.oos_return)
        if r.oos_return > 0:
            oos_profitable += 1

        print(
            f"  {r.window:>2} {r.is_start}→{r.is_end}  {r.oos_start}→{r.oos_end}"
            f"  {r.is_sharpe:>6.2f} {r.oos_sharpe:>6.2f} {wfe:>5.2f}"
            f"  {r.oos_pf:>6.2f} {r.oos_return:>6.1%} {r.oos_trades:>6}"
        )

    print(f"{'-'*90}")

    avg_wfe = sum(wfes) / len(wfes) if wfes else 0.0
    avg_oos_sharpe = sum(oos_sharpes) / len(oos_sharpes) if oos_sharpes else 0.0
    total_oos_return = 1.0
    for r in oos_returns:
        total_oos_return *= (1.0 + r)
    total_oos_return -= 1.0

    print(f"\n  Avg WFE:              {avg_wfe:.2f}  {'PASS' if avg_wfe > 0.50 else 'FAIL'} (threshold: >0.50)")
    print(f"  Avg OOS Sharpe:       {avg_oos_sharpe:.2f}")
    print(f"  OOS profitable:       {oos_profitable}/{len(results)} windows")
    print(f"  Compounded OOS return: {total_oos_return:.2%}")
    print(f"{'='*90}")


def main():
    parser = argparse.ArgumentParser(description="Walk-forward analysis (NautilusTrader)")
    parser.add_argument("--data-dir", type=Path, default=Path("data-crypto"))
    parser.add_argument("--tickers", help="Comma-separated tickers")
    parser.add_argument("--is-months", type=int, default=12, help="In-sample window months")
    parser.add_argument("--oos-months", type=int, default=6, help="Out-of-sample window months")
    parser.add_argument("--step-months", type=int, default=6, help="Roll step months")
    parser.add_argument("--first-date", default="2019-07-01", help="Earliest IS start date")
    parser.add_argument("--last-date", default="2023-12-31", help="Latest OOS end date")
    parser.add_argument("--generations", type=int, default=15)
    parser.add_argument("--pop-size", type=int, default=30)
    parser.add_argument("--workers", type=int, default=os.cpu_count())
    parser.add_argument("--fitness", default="sharpe")
    parser.add_argument("--output", help="Save full results to JSON")
    args = parser.parse_args()

    # Resolve tickers
    if args.tickers:
        tickers = [s.strip().upper() for s in args.tickers.split(",")]
    else:
        tickers = list(DEFAULT_TICKERS)
    if "BTCUSDT" not in tickers:
        tickers.insert(0, "BTCUSDT")

    windows = generate_windows(
        args.first_date, args.last_date,
        args.is_months, args.oos_months, args.step_months,
    )

    print(f"Walk-forward analysis: {len(windows)} windows", file=sys.stderr)
    print(f"  IS: {args.is_months}mo, OOS: {args.oos_months}mo, step: {args.step_months}mo", file=sys.stderr)
    print(f"  Range: {args.first_date} → {args.last_date}", file=sys.stderr)
    print(f"  Tickers: {len(tickers)}", file=sys.stderr)
    print(f"  Optimize: {args.generations} gen × {args.pop_size} pop, fitness={args.fitness}", file=sys.stderr)
    print(f"  Workers: {args.workers}", file=sys.stderr)

    for i, (is_s, is_e, oos_s, oos_e) in enumerate(windows):
        print(f"  Window {i+1}: IS {is_s}→{is_e}  OOS {oos_s}→{oos_e}", file=sys.stderr)

    t0 = time.time()
    results = run_walkforward(
        data_dir=args.data_dir.resolve(),
        tickers=tickers,
        windows=windows,
        generations=args.generations,
        pop_size=args.pop_size,
        n_workers=args.workers,
        fitness=args.fitness,
    )
    total = time.time() - t0

    print_summary(results)
    print(f"\nTotal time: {total:.0f}s ({total/60:.1f}min)")

    if args.output:
        out = []
        for r in results:
            out.append({
                "window": r.window,
                "is_start": r.is_start, "is_end": r.is_end,
                "oos_start": r.oos_start, "oos_end": r.oos_end,
                "is_sharpe": r.is_sharpe, "oos_sharpe": r.oos_sharpe,
                "is_pf": r.is_pf, "oos_pf": r.oos_pf,
                "is_trades": r.is_trades, "oos_trades": r.oos_trades,
                "is_return": r.is_return, "oos_return": r.oos_return,
                "oos_win_rate": r.oos_win_rate, "oos_max_dd": r.oos_max_dd,
                "best_params": r.best_params,
            })
        Path(args.output).write_text(json.dumps(out, indent=2))
        print(f"Saved to {args.output}")


if __name__ == "__main__":
    main()
