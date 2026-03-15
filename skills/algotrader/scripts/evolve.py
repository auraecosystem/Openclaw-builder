#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""
Evolutionary parameter optimizer for algotrader-engine.

Two modes:
  Subprocess: spawns engine per generation (8s overhead each time)
  Server:     connects to a running --serve instance (zero overhead)

Usage:
    # Start server (once, keep running):
    ./engine/target/release/algotrader-engine --profile equities_daily --serve --port 9999

    # Run optimizer against server:
    python3 scripts/evolve.py --server localhost:9999 --generations 50 --pop-size 100

    # Or subprocess mode (no server needed, slower):
    python3 scripts/evolve.py --profile equities_daily --generations 50 --pop-size 100
"""

import argparse
import json
import random
import socket
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from trading_config import load_trading_config


# Parameter ranges for mutation/crossover
PARAM_RANGES = {
    "rs_pct": (0.01, 0.20),
    "vol_ratio": (1.2, 3.0),
    "max_range_pct": (0.10, 0.30),
    "max_dist_52w": (0.10, 0.50),
    "min_adv": (5e6, 3e8),
    "slippage_k": (0.05, 0.20),
    "min_prior_move": (0.0, 0.50),
    "max_sma_ext": (0.05, 0.30),
    "risk_pct": (0.003, 0.010),
    "max_pos_pct": (0.10, 0.30),
    "split_frac": (0.05, 0.95),
    "min_adr_pct": (0.02, 0.20),
    "min_consol_days": (3, 20),
}

FIXED_PARAMS = {
    "setup": "breakout",
    "init_cash": 100_000.0,
}

FIXED_PARAMS_CRYPTO = {
    "setup": "breakout",
    "init_cash": 100_000.0,
    "bars_per_day": 1,  # set to 288 for 5m, 24 for 1h
    "min_price": 0.0,
    "min_vol": 0.0,
}

# Adjusted ranges for crypto (100-ticker universe, no penny stock filter)
PARAM_RANGES_CRYPTO = dict(PARAM_RANGES)
PARAM_RANGES_CRYPTO["min_adv"] = (0.0, 5e5)
PARAM_RANGES_CRYPTO["rs_pct"] = (0.20, 0.60)  # wider RS with only 100 tickers
PARAM_RANGES_CRYPTO["min_adr_pct"] = (0.0, 0.05)  # crypto is already volatile
PARAM_RANGES_CRYPTO["max_range_pct"] = (0.15, 0.50)  # wider range for crypto volatility
PARAM_RANGES_CRYPTO["min_prior_move"] = (0.0, 0.30)
PARAM_RANGES_CRYPTO["min_consol_days"] = (0, 10)  # lower max
PARAM_RANGES_CRYPTO["split_frac"] = (0.20, 0.80)
PARAM_RANGES_CRYPTO["max_hold_bars"] = (3, 15)  # time stop (bars)
PARAM_RANGES_CRYPTO["rs_lookback"] = (7, 63)  # RS ranking period
PARAM_RANGES_CRYPTO["flag_min_pole"] = (0.10, 0.40)
PARAM_RANGES_CRYPTO["flag_max_retrace"] = (0.25, 0.75)


INT_PARAMS = {"min_consol_days", "max_hold_bars", "rs_lookback"}


def random_params(crypto: bool = False) -> dict:
    p = dict(FIXED_PARAMS_CRYPTO if crypto else FIXED_PARAMS)
    ranges = PARAM_RANGES_CRYPTO if crypto else PARAM_RANGES
    for key, (lo, hi) in ranges.items():
        if key in INT_PARAMS:
            p[key] = random.randint(int(lo), int(hi))
        else:
            p[key] = round(random.uniform(lo, hi), 6)
    p["regime"] = random.choice([True, False])
    return p


def mutate(params: dict, rate: float = 0.3, sigma: float = 0.15, crypto: bool = False) -> dict:
    child = dict(params)
    ranges = PARAM_RANGES_CRYPTO if crypto else PARAM_RANGES
    for key, (lo, hi) in ranges.items():
        if random.random() < rate:
            val = child[key]
            noise = random.gauss(0, sigma * (hi - lo))
            new_val = max(lo, min(hi, val + noise))
            if key in INT_PARAMS:
                child[key] = int(round(new_val))
            else:
                child[key] = round(new_val, 6)
    if random.random() < rate:
        child["regime"] = not child["regime"]
    return child


def crossover(a: dict, b: dict) -> dict:
    child = {}
    for key in a:
        child[key] = a[key] if random.random() < 0.5 else b[key]
    return child


def fitness(report: dict, metric: str = "sharpe") -> float:
    trades = report.get("total_trades", 0)
    if trades < 10:
        return -999.0

    # Penalize very low trade counts — avoids degenerate overfitting.
    # Ramps from 0.3 at 10 trades to 1.0 at 50+ trades.
    trade_confidence = min(1.0, 0.3 + 0.7 * (trades - 10) / 40)

    if metric == "sharpe":
        score = report.get("sharpe", -999.0)
    elif metric == "sortino":
        score = report.get("sortino", -999.0)
    elif metric == "return":
        score = report.get("total_return", -999.0)
    elif metric == "profit_factor":
        score = report.get("profit_factor", 0.0)
    elif metric == "cagr":
        score = report.get("cagr", -999.0)
    elif metric == "growth":
        # end/start balance ratio — raw capital multiplier
        init = report.get("init_cash", 1.0)
        final = report.get("final_equity", init)
        score = final / init if init > 0 else -999.0
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


# --- Transport layer: server (TCP) or subprocess ---

class ServerTransport:
    """Send batches to a running --serve instance over TCP."""

    def __init__(self, host: str, port: int):
        self.host = host
        self.port = port
        self.sock = None
        self.rfile = None

    def connect(self):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.connect((self.host, self.port))
        self.rfile = self.sock.makefile("r")

    def close(self):
        if self.rfile:
            self.rfile.close()
        if self.sock:
            self.sock.close()

    def run_batch(self, params_list: list[dict]) -> list[dict]:
        if not self.sock:
            self.connect()
        payload = json.dumps(params_list) + "\n"
        self.sock.sendall(payload.encode())
        while True:
            line = self.rfile.readline()
            if not line:
                raise ConnectionError("Server closed connection")
            data = json.loads(line)
            # Progress updates are dicts with "progress" key; skip them
            if isinstance(data, dict) and "progress" in data:
                total = data.get("total", len(params_list))
                done = data["progress"]
                print(f"\r    processing: {done}/{total}", end="", flush=True, file=sys.stderr)
                continue
            print("", file=sys.stderr)  # newline after progress
            return data


class SubprocessTransport:
    """Spawn engine binary per batch (includes 8s load overhead each time)."""

    def __init__(self, binary: Path, profile: str):
        self.binary = binary
        self.profile = profile

    def connect(self):
        pass

    def close(self):
        pass

    def run_batch(self, params_list: list[dict]) -> list[dict]:
        params_file = Path("/tmp/algotrader-evolve-params.json")
        params_file.write_text(json.dumps(params_list))
        result = subprocess.run(
            [str(self.binary), "--profile", self.profile, "--batch", str(params_file)],
            capture_output=True,
            text=True,
            timeout=300,
        )
        if result.returncode != 0:
            print(f"  Engine error: {result.stderr[:500]}", file=sys.stderr)
            return [{"total_trades": 0}] * len(params_list)
        return json.loads(result.stdout)


def _tournament_select(scored: list, k: int = 3) -> dict:
    """Pick k random individuals, return the best one's params."""
    candidates = random.sample(scored, min(k, len(scored)))
    return candidates[0][0]  # scored is already sorted by fitness


def _population_diversity(population: list[dict], ranges: dict) -> float:
    """Measure population diversity as mean normalized standard deviation across params.

    Returns 0.0 (all clones) to ~0.29 (uniform random). Used to detect convergence.
    """
    if len(population) < 2:
        return 0.0
    diversities = []
    for key, (lo, hi) in ranges.items():
        span = hi - lo
        if span <= 0:
            continue
        vals = [p.get(key, lo) for p in population]
        mean = sum(vals) / len(vals)
        var = sum((v - mean) ** 2 for v in vals) / len(vals)
        diversities.append(var ** 0.5 / span)
    return sum(diversities) / len(diversities) if diversities else 0.0


def evolve(
    transport,
    generations: int = 50,
    pop_size: int = 100,
    elite_frac: float = 0.25,
    metric: str = "sharpe",
    crypto: bool = False,
) -> tuple[dict | None, dict | None]:
    ranges = PARAM_RANGES_CRYPTO if crypto else PARAM_RANGES
    population = [random_params(crypto=crypto) for _ in range(pop_size)]
    best_score = -999.0
    best_params = None
    best_report = None
    elite_count = max(2, int(pop_size * elite_frac))

    # Adaptive mutation state
    base_sigma = 0.15
    sigma = base_sigma
    stale_gens = 0
    stale_limit = 5       # gens without improvement before triggering injection
    cataclysm_limit = 12  # deep stagnation triggers population reset

    for gen in range(generations):
        t0 = time.time()
        reports = transport.run_batch(population)
        elapsed = time.time() - t0

        scored = sorted(
            zip(population, reports),
            key=lambda pr: fitness(pr[1], metric),
            reverse=True,
        )

        top_score = fitness(scored[0][1], metric)
        top_trades = scored[0][1].get("total_trades", 0)
        top_return = scored[0][1].get("total_return", 0.0)
        top_cagr = scored[0][1].get("cagr", 0.0)

        improved = top_score > best_score
        if improved:
            best_score = top_score
            best_params = scored[0][0]
            best_report = scored[0][1]
            stale_gens = 0
            sigma = base_sigma  # reset mutation strength on improvement
        else:
            stale_gens += 1
            # Ramp up mutation as stagnation grows (escape local optimum)
            sigma = base_sigma * (1.0 + stale_gens * 0.3)

        diversity = _population_diversity(population, ranges)

        status = ""
        if stale_gens >= cataclysm_limit:
            status = " CATACLYSM"
        elif stale_gens >= stale_limit:
            status = " inject"
        elif improved:
            status = " *"

        print(
            f"  Gen {gen+1:3d}/{generations}: "
            f"best {metric}={top_score:.4f} trades={top_trades} ret={top_return:.4f} "
            f"cagr={top_cagr:.4f} "
            f"div={diversity:.3f} σ={sigma:.2f}"
            f"{status}"
            f" ({elapsed:.1f}s)",
            file=sys.stderr,
        )

        if gen == generations - 1:
            break

        # --- Build next generation ---
        # Always keep the global best and generation elite
        next_pop = [dict(best_params)] if best_params else []
        elite = [p for p, _ in scored[:elite_count]]
        next_pop.extend(elite)

        # Cataclysm: deep stagnation — nuke 50% of slots with random individuals
        if stale_gens >= cataclysm_limit:
            n_random = pop_size // 2
            stale_gens = 0  # reset counter after cataclysm
            sigma = base_sigma * 2.0  # high mutation for survivors too
        # Stagnation injection: replace 30% with random to add diversity
        elif stale_gens >= stale_limit:
            n_random = int(pop_size * 0.3)
        else:
            n_random = 0

        for _ in range(n_random):
            next_pop.append(random_params(crypto=crypto))

        # Fill remaining slots via tournament selection + crossover + mutation
        while len(next_pop) < pop_size:
            a = _tournament_select(scored)
            b = _tournament_select(scored)
            child = crossover(a, b)
            child = mutate(child, rate=0.3, sigma=sigma, crypto=crypto)
            next_pop.append(child)

        # Trim to exact pop_size (elite + random injection can overshoot)
        population = next_pop[:pop_size]

    return best_params, best_report


def main():
    parser = argparse.ArgumentParser(description="Evolutionary parameter optimizer")
    parser.add_argument("--server", default=None, help="Connect to running server (host:port, e.g. localhost:9999)")
    parser.add_argument("--profile", default="equities_daily", help="Configured dataset profile")
    parser.add_argument("--binary", default=None, help="Path to algotrader-engine binary")
    parser.add_argument("--generations", type=int, default=50)
    parser.add_argument("--pop-size", type=int, default=100)
    parser.add_argument("--elite-frac", type=float, default=0.25)
    parser.add_argument("--fitness", default="composite", help="sharpe|sortino|return|profit_factor|cagr|growth|composite")
    parser.add_argument("--output", default=None, help="Output JSON path for best result")
    parser.add_argument("--crypto", action="store_true", help="Use crypto defaults (min_price=0, min_vol=100k, bars_per_day=288)")
    args = parser.parse_args()

    # Build transport
    if args.server:
        host, port = args.server.split(":")
        transport = ServerTransport(host, int(port))
        print(f"Server mode: {args.server}", file=sys.stderr)
    else:
        cfg = load_trading_config()
        if args.binary:
            binary = Path(args.binary)
        else:
            binary = cfg.engine_binary
            if not binary.exists():
                debug_binary = cfg.repo_root / "engine" / "target" / "debug" / "algotrader-engine"
                if debug_binary.exists():
                    binary = debug_binary
        if not binary.exists():
            print(f"Binary not found: {binary}", file=sys.stderr)
            print("Build with: cd engine && cargo build --release", file=sys.stderr)
            sys.exit(1)
        transport = SubprocessTransport(binary, args.profile)
        print(f"Subprocess mode: {binary}", file=sys.stderr)

    if args.crypto:
        print("Crypto mode: min_price=0, min_vol=0, wider filter ranges", file=sys.stderr)
    print(f"Generations: {args.generations}, Pop: {args.pop_size}, Fitness: {args.fitness}", file=sys.stderr)

    transport.connect()
    t0 = time.time()
    try:
        best_params, best_report = evolve(
            transport=transport,
            generations=args.generations,
            pop_size=args.pop_size,
            elite_frac=args.elite_frac,
            metric=args.fitness,
            crypto=args.crypto,
        )
    finally:
        transport.close()

    total = time.time() - t0
    total_runs = args.generations * args.pop_size
    print(f"\nDone in {total:.1f}s ({total_runs} backtests, {total/total_runs*1000:.0f}ms/run avg)", file=sys.stderr)

    result = {
        "best_params": best_params,
        "best_report": best_report,
    }

    if args.output:
        Path(args.output).write_text(json.dumps(result, indent=2))
        print(f"Saved to {args.output}", file=sys.stderr)
    else:
        print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
