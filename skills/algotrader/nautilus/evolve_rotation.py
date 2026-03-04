"""CMA-ES evolutionary optimizer for the RotationStrategy.

Uses the Python `cma` package (Hansen 2016) to optimize RotationConfig
parameters. Each fitness evaluation runs a full NT backtest via a
multiprocessing worker pool that pre-loads data and reuses engines.

Usage:
    python -m nautilus.evolve_rotation --data-dir data/
    python -m nautilus.evolve_rotation --data-dir data/ --generations 50 --pop-size 20
    python -m nautilus.evolve_rotation --data-dir data/ --venue BINANCE --currency USDT
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
import time
from multiprocessing import Pool
from pathlib import Path

import cma
import numpy as np

_project_root = str(Path(__file__).resolve().parent.parent)
if _project_root not in sys.path:
    sys.path.insert(0, _project_root)

from nautilus.genome import ROTATION_GENOME, GeneBound, decode, encode, genome_midpoint, genome_sigma

# ---------------------------------------------------------------------------
# Worker globals — initialized once per process
# ---------------------------------------------------------------------------

_w_engine = None
_w_instrument_ids: list[str] = []
_w_bar_spec: str = ""
_w_venue: str = ""
_w_currency: str = ""
_w_cache_dir: str = ""
_w_init_cash: float = 1_000_000.0


def _init_worker(
    data_dir_str: str,
    venue: str,
    currency: str,
    bar_spec: str,
    cash: float,
) -> None:
    """Pool initializer: load data and build a reusable BacktestEngine."""
    global _w_engine, _w_instrument_ids, _w_bar_spec, _w_venue, _w_currency, _w_cache_dir, _w_init_cash

    from decimal import Decimal

    from nautilus_trader.backtest.engine import BacktestEngine
    from nautilus_trader.config import BacktestEngineConfig, LoggingConfig
    from nautilus_trader.model.enums import AccountType, OmsType
    from nautilus_trader.model.identifiers import TraderId, Venue
    from nautilus_trader.model.objects import Currency, Money

    from nautilus.run_rotation import load_wide_parquet_data

    data_dir = Path(data_dir_str)
    _w_bar_spec = bar_spec
    _w_venue = venue
    _w_currency = currency
    _w_cache_dir = str(data_dir / "cache")
    _w_init_cash = cash

    quote_currency = Currency.from_str(currency)

    data = load_wide_parquet_data(data_dir, venue, bar_spec)
    if not data:
        return

    engine = BacktestEngine(
        config=BacktestEngineConfig(
            trader_id=TraderId("EVOLVE-ROT-001"),
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

    ids = []
    for instrument, bt, bars in data:
        engine.add_instrument(instrument)
        engine.add_data(bars, sort=False)
        ids.append(str(instrument.id))

    engine.sort_data()

    _w_engine = engine
    _w_instrument_ids = ids


def _run_one(genome_vec: list[float]) -> dict:
    """Decode genome, run backtest, return report dict."""
    if _w_engine is None:
        return _empty_report(genome_vec)

    from nautilus_trader.model.identifiers import Venue
    from nautilus_trader.model.objects import Currency

    from nautilus.rotation_strategy import RotationConfig, RotationStrategy

    params = decode(genome_vec, ROTATION_GENOME)

    engine = _w_engine
    engine.reset()
    engine.clear_strategies()

    config = RotationConfig(
        instrument_ids=_w_instrument_ids,
        bar_spec=_w_bar_spec,
        cache_dir=_w_cache_dir,
        venue=_w_venue,
        quote_currency=_w_currency,
        top_n=params["top_n"],
        rebalance_bars=params["rebalance_bars"],
        min_score=params["min_score"],
        w_rs=params["w_rs"],
        w_pattern=params["w_pattern"],
        w_signal=params["w_signal"],
        risk_pct=params["risk_pct"],
        max_pos_pct=params["max_pos_pct"],
        stop_atr_mult=params["stop_atr_mult"],
    )

    strategy = RotationStrategy(config=config)
    engine.add_strategy(strategy)

    try:
        engine.run()
    except Exception:
        return _empty_report(genome_vec)

    return _extract_report(engine, params, genome_vec)


def _extract_report(engine, params: dict, genome_vec: list[float]) -> dict:
    """Pull metrics from the engine after a run."""
    from nautilus_trader.model.identifiers import Venue
    from nautilus_trader.model.objects import Currency

    result = engine.get_result()
    venue = Venue(_w_venue)
    currency = Currency.from_str(_w_currency)

    try:
        account = engine.kernel.portfolio.account(venue)
        final_equity = float(account.balance_total(currency))
    except Exception:
        final_equity = _w_init_cash

    total_return = (final_equity - _w_init_cash) / _w_init_cash if _w_init_cash > 0 else 0.0

    if result.backtest_start and result.backtest_end:
        seconds = (result.backtest_end - result.backtest_start) / 1e9
        years = seconds / (365.25 * 86400)
    else:
        years = 1.0

    if years > 0 and total_return > -1.0:
        cagr = (1.0 + total_return) ** (1.0 / years) - 1.0
    else:
        cagr = 0.0

    # Max drawdown from portfolio returns
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

    pnl_stats = result.stats_pnls.get(_w_currency, {})
    ret_stats = result.stats_returns or {}

    return {
        "total_trades": result.total_positions,
        "win_rate": _float(pnl_stats.get("Win Rate", 0.0)),
        "profit_factor": _float(ret_stats.get("Profit Factor", 0.0)),
        "sharpe": _float(ret_stats.get("Sharpe Ratio (252 days)", 0.0)),
        "sortino": _float(ret_stats.get("Sortino Ratio (252 days)", 0.0)),
        "total_return": total_return,
        "cagr": cagr,
        "max_drawdown": max_drawdown,
        "init_cash": _w_init_cash,
        "final_equity": final_equity,
        "params": params,
        "genome": genome_vec,
    }


def _float(v) -> float:
    if v is None:
        return 0.0
    try:
        return float(v)
    except (TypeError, ValueError):
        return 0.0


def _empty_report(genome_vec: list[float]) -> dict:
    return {
        "total_trades": 0,
        "win_rate": 0.0,
        "profit_factor": 0.0,
        "sharpe": 0.0,
        "sortino": 0.0,
        "total_return": 0.0,
        "cagr": 0.0,
        "max_drawdown": 0.0,
        "init_cash": _w_init_cash,
        "final_equity": _w_init_cash,
        "params": {},
        "genome": genome_vec,
    }


# ---------------------------------------------------------------------------
# CMA-ES fitness
# ---------------------------------------------------------------------------

def cma_fitness(report: dict, metric: str = "sharpe") -> float:
    """Compute fitness score (higher = better). CMA-ES minimizes, so negate."""
    trades = report.get("total_trades", 0)
    if trades < 10:
        return -999.0

    # Confidence ramp: 0.3 at 10 trades → 1.0 at 50+ trades
    trade_confidence = min(1.0, 0.3 + 0.7 * (trades - 10) / 40)

    if metric == "sharpe":
        score = report.get("sharpe", -999.0)
    elif metric == "sortino":
        score = report.get("sortino", -999.0)
    elif metric == "cagr":
        score = report.get("cagr", -999.0)
    elif metric == "composite":
        sharpe = report.get("sharpe", 0.0)
        ret = report.get("total_return", 0.0)
        wr = report.get("win_rate", 0.0)
        dd = report.get("max_drawdown", 1.0)
        dd_penalty = max(0.0, 1.0 - dd * 2)
        score = (sharpe * 0.4 + ret * 10.0 * 0.3 + wr * 0.3) * dd_penalty
    else:
        score = report.get(metric, -999.0)

    return score * trade_confidence


# ---------------------------------------------------------------------------
# CMA-ES transport
# ---------------------------------------------------------------------------

class RotationTransport:
    """Run RotationStrategy backtests in a multiprocessing pool."""

    def __init__(
        self,
        data_dir: Path,
        venue: str = "XNYS",
        currency: str = "USD",
        bar_spec: str = "1-DAY-LAST",
        cash: float = 1_000_000.0,
        n_workers: int | None = None,
    ):
        self.data_dir = data_dir.resolve()
        self.venue = venue
        self.currency = currency
        self.bar_spec = bar_spec
        self.cash = cash
        self.n_workers = n_workers or os.cpu_count() or 4
        self.pool: Pool | None = None

    def connect(self) -> None:
        self.pool = Pool(
            processes=self.n_workers,
            initializer=_init_worker,
            initargs=(
                str(self.data_dir), self.venue, self.currency,
                self.bar_spec, self.cash,
            ),
        )

    def close(self) -> None:
        if self.pool:
            self.pool.terminate()
            self.pool.join()
            self.pool = None

    def run_batch(self, genome_vecs: list[list[float]]) -> list[dict]:
        if not self.pool:
            self.connect()
        return self.pool.map(_run_one, genome_vecs)


# ---------------------------------------------------------------------------
# CMA-ES loop
# ---------------------------------------------------------------------------

def evolve_cma(
    transport: RotationTransport,
    genome_spec: list[GeneBound] = ROTATION_GENOME,
    generations: int = 50,
    pop_size: int = 20,
    metric: str = "sharpe",
    seed_params: dict | None = None,
) -> tuple[dict | None, dict | None]:
    """Run CMA-ES optimization, return (best_params, best_report)."""
    # Initial mean: midpoint or seeded
    if seed_params:
        x0 = encode(seed_params, genome_spec)
    else:
        x0 = genome_midpoint(genome_spec)

    sigma0 = genome_sigma()
    bounds = [[g.min_val for g in genome_spec], [g.max_val for g in genome_spec]]

    opts = cma.CMAOptions()
    opts.set("bounds", bounds)
    opts.set("popsize", pop_size)
    opts.set("maxiter", generations)
    opts.set("verbose", -1)  # suppress CMA-ES internal output
    opts.set("seed", 42)

    es = cma.CMAEvolutionStrategy(x0, sigma0, opts)

    best_score = -999.0
    best_params = None
    best_report = None

    gen = 0
    while not es.stop():
        gen += 1
        t0 = time.time()

        solutions = es.ask()
        reports = transport.run_batch([list(s) for s in solutions])

        # CMA-ES minimizes — negate fitness
        fitnesses = [-cma_fitness(r, metric) for r in reports]
        es.tell(solutions, fitnesses)

        # Track best
        gen_best_idx = int(np.argmin(fitnesses))
        gen_best_score = -fitnesses[gen_best_idx]
        gen_best_report = reports[gen_best_idx]
        elapsed = time.time() - t0

        improved = gen_best_score > best_score
        if improved:
            best_score = gen_best_score
            best_params = gen_best_report.get("params", {})
            best_report = gen_best_report

        trades = gen_best_report.get("total_trades", 0)
        ret = gen_best_report.get("total_return", 0.0)
        sharpe = gen_best_report.get("sharpe", 0.0)

        marker = "*" if improved else " "
        print(
            f"  Gen {gen:3d}/{generations} {marker} "
            f"score={gen_best_score:+.4f}  "
            f"trades={trades:4d}  ret={ret:+.1%}  sharpe={sharpe:.2f}  "
            f"({elapsed:.1f}s, {len(solutions)} evals)",
            file=sys.stderr,
        )

    return best_params, best_report


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="CMA-ES optimizer for RotationStrategy")
    parser.add_argument("--data-dir", type=Path, required=True)
    parser.add_argument("--venue", default="XNYS")
    parser.add_argument("--currency", default="USD")
    parser.add_argument("--bar-spec", default="1-DAY-LAST")
    parser.add_argument("--cash", type=float, default=1_000_000)
    parser.add_argument("--workers", type=int, default=os.cpu_count())
    parser.add_argument("--generations", type=int, default=50)
    parser.add_argument("--pop-size", type=int, default=20)
    parser.add_argument("--fitness", default="sharpe", help="Fitness metric: sharpe, sortino, cagr, composite")
    parser.add_argument("--seed", help="JSON file with seed params to start from")
    parser.add_argument("--output", help="Save best result to JSON")
    args = parser.parse_args()

    seed_params = None
    if args.seed:
        seed_params = json.loads(Path(args.seed).read_text())

    print("CMA-ES rotation optimizer", file=sys.stderr)
    print(f"  Data: {args.data_dir}", file=sys.stderr)
    print(f"  Venue: {args.venue}, Currency: {args.currency}", file=sys.stderr)
    print(f"  Workers: {args.workers}, Generations: {args.generations}, Pop: {args.pop_size}", file=sys.stderr)
    print(f"  Fitness: {args.fitness}", file=sys.stderr)

    transport = RotationTransport(
        data_dir=args.data_dir,
        venue=args.venue,
        currency=args.currency,
        bar_spec=args.bar_spec,
        cash=args.cash,
        n_workers=args.workers,
    )

    transport.connect()
    t0 = time.time()
    try:
        best_params, best_report = evolve_cma(
            transport=transport,
            generations=args.generations,
            pop_size=args.pop_size,
            metric=args.fitness,
            seed_params=seed_params,
        )
    finally:
        transport.close()

    total = time.time() - t0
    print(f"\nDone in {total:.1f}s", file=sys.stderr)

    if best_params:
        print(f"\nBest params:", file=sys.stderr)
        for k, v in best_params.items():
            print(f"  {k}: {v}", file=sys.stderr)
        print(f"\nBest report:", file=sys.stderr)
        for k, v in best_report.items():
            if k not in ("params", "genome"):
                print(f"  {k}: {v}", file=sys.stderr)

    result = {"best_params": best_params, "best_report": best_report}

    if args.output:
        Path(args.output).write_text(json.dumps(result, indent=2))
        print(f"\nSaved to {args.output}", file=sys.stderr)
    else:
        print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
