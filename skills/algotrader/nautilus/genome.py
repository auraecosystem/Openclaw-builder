"""Shared genome encoding for CMA-ES and GA evolutionary optimization.

Translates between flat float vectors (optimizer space) and typed parameter
dicts (strategy config space). Handles continuous, integer, and boolean genes
with bounds clamping.
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class GeneBound:
    """One dimension of the genome search space."""
    name: str
    min_val: float
    max_val: float
    is_integer: bool = False
    is_boolean: bool = False


# Genome spec for RotationConfig optimization
ROTATION_GENOME: list[GeneBound] = [
    GeneBound("w_rs",               0.05,  0.80),
    GeneBound("w_pattern",          0.00,  0.60),
    GeneBound("w_signal",           0.00,  0.60),
    GeneBound("top_n",              3,     50,    is_integer=True),
    GeneBound("rebalance_bars",     1,     20,    is_integer=True),
    GeneBound("min_score",          0.00,  0.50),
    GeneBound("risk_pct",           0.002, 0.03),
    GeneBound("max_pos_pct",        0.05,  0.40),
    GeneBound("stop_atr_mult",      0.5,   4.0),
]

# Genome spec for Qullamaggie breakout optimization
BREAKOUT_GENOME: list[GeneBound] = [
    GeneBound("rs_pct",             0.05,  0.50),
    GeneBound("max_dist_52w",       0.05,  0.50),
    GeneBound("min_prior_move",     0.10,  0.80),
    GeneBound("min_adr_pct",        0.01,  0.10),
    GeneBound("vol_spike",          1.0,   4.0),
    GeneBound("max_range_pct",      0.05,  0.30),
    GeneBound("risk_pct",           0.002, 0.03),
    GeneBound("max_pos_pct",        0.10,  0.40),
    GeneBound("split_frac",         0.0,   1.0),
    GeneBound("trail_period",       10,    20,    is_integer=True),
    GeneBound("max_hold_bars",      0,     40,    is_integer=True),
    GeneBound("rs_lookback",        7,     126,   is_integer=True),
]


def genome_midpoint(spec: list[GeneBound]) -> list[float]:
    """Initial mean for CMA-ES: midpoint of each gene's range."""
    return [(g.min_val + g.max_val) / 2.0 for g in spec]


def genome_sigma(frac: float = 0.3) -> float:
    """Initial step size: fraction of the average normalized range."""
    return frac


def decode(x: list[float], spec: list[GeneBound]) -> dict[str, float | int | bool]:
    """Decode a CMA-ES sample vector to a parameter dict.

    Clamps to bounds, rounds integers, thresholds booleans at 0.5.
    """
    params = {}
    for val, gene in zip(x, spec):
        clamped = max(gene.min_val, min(gene.max_val, val))
        if gene.is_boolean:
            params[gene.name] = clamped >= 0.5
        elif gene.is_integer:
            params[gene.name] = int(round(clamped))
        else:
            params[gene.name] = clamped
    return params


def encode(params: dict, spec: list[GeneBound]) -> list[float]:
    """Encode a parameter dict back to a flat vector (for seeding)."""
    return [float(params.get(g.name, (g.min_val + g.max_val) / 2)) for g in spec]
