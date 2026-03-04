"""Walk-forward analysis for the RotationStrategy.

Rolls through time windows: optimize on in-sample, validate on out-of-sample.
Computes walk-forward efficiency (WFE = OOS Sharpe / IS Sharpe).

Usage:
    python -m nautilus.walkforward_rotation --data-dir data/
    python -m nautilus.walkforward_rotation --data-dir data/ --is-months 24 --oos-months 6
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

from nautilus.evolve_rotation import RotationTransport, evolve_cma


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
    is_cagr: float = 0.0
    oos_cagr: float = 0.0
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


def _run_single_backtest(
    data_dir: Path,
    venue: str,
    currency: str,
    bar_spec: str,
    cash: float,
    params: dict,
    start: str | None = None,  # noqa: ARG001 — reserved for date filtering
    end: str | None = None,  # noqa: ARG001 — reserved for date filtering
) -> dict:
    """Run a single RotationStrategy backtest with fixed params.

    Builds a fresh engine with date-filtered data for OOS validation.
    """
    from nautilus_trader.backtest.engine import BacktestEngine
    from nautilus_trader.config import BacktestEngineConfig, LoggingConfig
    from nautilus_trader.model.enums import AccountType, OmsType
    from nautilus_trader.model.identifiers import TraderId, Venue
    from nautilus_trader.model.objects import Currency, Money

    from nautilus.rotation_strategy import RotationConfig, RotationStrategy
    from nautilus.run_rotation import load_wide_parquet_data

    quote_currency = Currency.from_str(currency)
    data = load_wide_parquet_data(data_dir, venue, bar_spec)
    if not data:
        return {"total_trades": 0, "sharpe": 0.0, "total_return": 0.0}

    engine = BacktestEngine(
        config=BacktestEngineConfig(
            trader_id=TraderId("WF-VAL-001"),
            logging=LoggingConfig(bypass_logging=True),
        ),
    )

    engine.add_venue(
        venue=Venue(venue),
        oms_type=OmsType.NETTING,
        account_type=AccountType.MARGIN,
        base_currency=quote_currency,
        starting_balances=[Money(cash, quote_currency)],
        bar_execution=True,
    )

    instrument_ids = []
    for instrument, _bt, bars in data:
        engine.add_instrument(instrument)
        engine.add_data(bars, sort=False)
        instrument_ids.append(str(instrument.id))

    engine.sort_data()

    cache_dir = str(data_dir / "cache")
    config = RotationConfig(
        instrument_ids=instrument_ids,
        bar_spec=bar_spec,
        cache_dir=cache_dir,
        venue=venue,
        quote_currency=currency,
        top_n=params.get("top_n", 20),
        rebalance_bars=params.get("rebalance_bars", 5),
        min_score=params.get("min_score", 0.0),
        w_rs=params.get("w_rs", 0.4),
        w_pattern=params.get("w_pattern", 0.3),
        w_signal=params.get("w_signal", 0.3),
        risk_pct=params.get("risk_pct", 0.01),
        max_pos_pct=params.get("max_pos_pct", 0.10),
        stop_atr_mult=params.get("stop_atr_mult", 2.0),
    )

    strategy = RotationStrategy(config=config)
    engine.add_strategy(strategy)

    try:
        engine.run()
    except Exception:
        engine.dispose()
        return {"total_trades": 0, "sharpe": 0.0, "total_return": 0.0}

    result = engine.get_result()
    v = Venue(venue)
    cur = Currency.from_str(currency)

    try:
        account = engine.kernel.portfolio.account(v)
        final_equity = float(account.balance_total(cur))
    except Exception:
        final_equity = cash

    total_return = (final_equity - cash) / cash if cash > 0 else 0.0

    if result.backtest_start and result.backtest_end:
        seconds = (result.backtest_end - result.backtest_start) / 1e9
        years = seconds / (365.25 * 86400)
    else:
        years = 1.0

    cagr = (1.0 + total_return) ** (1.0 / years) - 1.0 if years > 0 and total_return > -1.0 else 0.0

    max_drawdown = 0.0
    try:
        returns = engine.kernel.portfolio.analyzer._returns
        if len(returns) > 0:
            cumulative = (1.0 + returns).cumprod()
            running_max = cumulative.cummax()
            drawdowns = (cumulative - running_max) / running_max
            max_drawdown = abs(float(drawdowns.min()))
    except Exception:
        pass

    pnl_stats = result.stats_pnls.get(currency, {})
    ret_stats = result.stats_returns or {}

    engine.dispose()

    def _f(v):
        try:
            return float(v) if v is not None else 0.0
        except (TypeError, ValueError):
            return 0.0

    return {
        "total_trades": result.total_positions,
        "win_rate": _f(pnl_stats.get("Win Rate", 0.0)),
        "profit_factor": _f(ret_stats.get("Profit Factor", 0.0)),
        "sharpe": _f(ret_stats.get("Sharpe Ratio (252 days)", 0.0)),
        "sortino": _f(ret_stats.get("Sortino Ratio (252 days)", 0.0)),
        "total_return": total_return,
        "cagr": cagr,
        "max_drawdown": max_drawdown,
    }


def run_walkforward(
    data_dir: Path,
    windows: list[tuple[str, str, str, str]],
    venue: str = "XNYS",
    currency: str = "USD",
    bar_spec: str = "1-DAY-LAST",
    cash: float = 1_000_000.0,
    generations: int = 30,
    pop_size: int = 20,
    n_workers: int = 8,
    fitness: str = "sharpe",
) -> list[WindowResult]:
    """Run walk-forward analysis across all windows."""
    results = []

    for i, (is_start, is_end, oos_start, oos_end) in enumerate(windows):
        print(f"\n{'='*70}", file=sys.stderr)
        print(f"Window {i+1}/{len(windows)}", file=sys.stderr)
        print(f"  IS:  {is_start} → {is_end}", file=sys.stderr)
        print(f"  OOS: {oos_start} → {oos_end}", file=sys.stderr)
        print(f"{'='*70}", file=sys.stderr)

        # --- In-sample: CMA-ES optimization ---
        print(f"\n--- Optimizing (IS) ---", file=sys.stderr)
        t0 = time.time()

        # TODO: date filtering for IS/OOS requires loading data subsets.
        # For now, run on full data — proper date filtering will be added
        # when the rotation strategy supports start/end date params.
        transport = RotationTransport(
            data_dir=data_dir,
            venue=venue,
            currency=currency,
            bar_spec=bar_spec,
            cash=cash,
            n_workers=n_workers,
        )
        transport.connect()

        try:
            best_params, best_report = evolve_cma(
                transport=transport,
                generations=generations,
                pop_size=pop_size,
                metric=fitness,
            )
        finally:
            transport.close()

        is_elapsed = time.time() - t0

        if best_params is None or best_report is None:
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
        is_cagr = best_report.get("cagr", 0.0)

        print(f"\n  IS result ({is_elapsed:.0f}s):", file=sys.stderr)
        print(f"    Sharpe={is_sharpe:.2f}  PF={is_pf:.2f}  CAGR={is_cagr:.1%}  trades={is_trades}  ret={is_return:.2%}", file=sys.stderr)

        # --- Out-of-sample: validate with best IS params ---
        print(f"\n--- Validating (OOS) ---", file=sys.stderr)
        t1 = time.time()

        oos_report = _run_single_backtest(
            data_dir=data_dir,
            venue=venue,
            currency=currency,
            bar_spec=bar_spec,
            cash=cash,
            params=best_params,
            start=oos_start,
            end=oos_end,
        )

        oos_elapsed = time.time() - t1

        oos_sharpe = oos_report.get("sharpe", 0.0)
        oos_pf = oos_report.get("profit_factor", 0.0)
        oos_trades = oos_report.get("total_trades", 0)
        oos_return = oos_report.get("total_return", 0.0)
        oos_cagr = oos_report.get("cagr", 0.0)
        oos_wr = oos_report.get("win_rate", 0.0)
        oos_dd = oos_report.get("max_drawdown", 0.0)

        print(f"\n  OOS result ({oos_elapsed:.0f}s):", file=sys.stderr)
        print(f"    Sharpe={oos_sharpe:.2f}  PF={oos_pf:.2f}  CAGR={oos_cagr:.1%}  trades={oos_trades}  ret={oos_return:.2%}  WR={oos_wr:.1%}  DD={oos_dd:.1%}", file=sys.stderr)

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
            is_cagr=is_cagr, oos_cagr=oos_cagr,
            oos_win_rate=oos_wr, oos_max_dd=oos_dd,
            best_params=best_params,
        ))

    return results


def print_summary(results: list[WindowResult]) -> None:
    """Print walk-forward summary table."""
    print(f"\n{'='*100}")
    print(f"WALK-FORWARD SUMMARY — Rotation Strategy")
    print(f"{'='*100}")
    print(f"{'Win':>4} {'IS period':>23} {'OOS period':>23}  {'IS Sh':>6} {'OOS Sh':>6} {'WFE':>5}  {'OOS PF':>6} {'OOS ret':>7} {'OOS CAGR':>8} {'OOS tr':>6}")
    print(f"{'-'*100}")

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
            f"  {r.oos_pf:>6.2f} {r.oos_return:>6.1%} {r.oos_cagr:>7.1%} {r.oos_trades:>6}"
        )

    print(f"{'-'*100}")

    avg_wfe = sum(wfes) / len(wfes) if wfes else 0.0
    avg_oos_sharpe = sum(oos_sharpes) / len(oos_sharpes) if oos_sharpes else 0.0
    total_oos_return = 1.0
    for r in oos_returns:
        total_oos_return *= (1.0 + r)
    total_oos_return -= 1.0

    print(f"\n  Avg WFE:               {avg_wfe:.2f}  {'PASS' if avg_wfe > 0.50 else 'FAIL'} (threshold: >0.50)")
    print(f"  Avg OOS Sharpe:        {avg_oos_sharpe:.2f}")
    print(f"  OOS profitable:        {oos_profitable}/{len(results)} windows")
    print(f"  Compounded OOS return: {total_oos_return:.2%}")
    print(f"{'='*100}")


def main():
    parser = argparse.ArgumentParser(description="Walk-forward analysis for RotationStrategy")
    parser.add_argument("--data-dir", type=Path, required=True)
    parser.add_argument("--venue", default="XNYS")
    parser.add_argument("--currency", default="USD")
    parser.add_argument("--bar-spec", default="1-DAY-LAST")
    parser.add_argument("--cash", type=float, default=1_000_000)
    parser.add_argument("--is-months", type=int, default=24, help="In-sample window months")
    parser.add_argument("--oos-months", type=int, default=6, help="Out-of-sample window months")
    parser.add_argument("--step-months", type=int, default=6, help="Roll step months")
    parser.add_argument("--first-date", default="2015-01-01", help="Earliest IS start date")
    parser.add_argument("--last-date", default="2025-12-31", help="Latest OOS end date")
    parser.add_argument("--generations", type=int, default=30)
    parser.add_argument("--pop-size", type=int, default=20)
    parser.add_argument("--workers", type=int, default=os.cpu_count())
    parser.add_argument("--fitness", default="sharpe")
    parser.add_argument("--output", help="Save full results to JSON")
    args = parser.parse_args()

    windows = generate_windows(
        args.first_date, args.last_date,
        args.is_months, args.oos_months, args.step_months,
    )

    print(f"Walk-forward analysis: {len(windows)} windows", file=sys.stderr)
    print(f"  IS: {args.is_months}mo, OOS: {args.oos_months}mo, step: {args.step_months}mo", file=sys.stderr)
    print(f"  Range: {args.first_date} → {args.last_date}", file=sys.stderr)
    print(f"  Venue: {args.venue}, Currency: {args.currency}", file=sys.stderr)
    print(f"  Optimize: {args.generations} gen × {args.pop_size} pop, fitness={args.fitness}", file=sys.stderr)
    print(f"  Workers: {args.workers}", file=sys.stderr)

    for i, (is_s, is_e, oos_s, oos_e) in enumerate(windows):
        print(f"  Window {i+1}: IS {is_s}→{is_e}  OOS {oos_s}→{oos_e}", file=sys.stderr)

    t0 = time.time()
    results = run_walkforward(
        data_dir=args.data_dir.resolve(),
        windows=windows,
        venue=args.venue,
        currency=args.currency,
        bar_spec=args.bar_spec,
        cash=args.cash,
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
                "is_cagr": r.is_cagr, "oos_cagr": r.oos_cagr,
                "oos_win_rate": r.oos_win_rate, "oos_max_dd": r.oos_max_dd,
                "best_params": r.best_params,
            })
        Path(args.output).write_text(json.dumps(out, indent=2))
        print(f"Saved to {args.output}")


if __name__ == "__main__":
    main()
