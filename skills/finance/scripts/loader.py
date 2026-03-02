"""Load and deduplicate Simplii Financial CSV transaction data."""

import csv
import glob


def _parse_float(val: str) -> float:
    if not val or not val.strip():
        return 0.0
    return float(val.replace(",", ""))


def _dedup(rows: list[dict], key_fields: list[str]) -> list[dict]:
    seen = set()
    out = []
    for r in rows:
        key = tuple(str(r.get(k, "")) for k in key_fields)
        if key not in seen:
            seen.add(key)
            out.append(r)
    return out


def load_chequing(data_dir: str) -> list[dict]:
    """Load all chequing CSVs, parse amounts, deduplicate."""
    rows = []
    for path in sorted(glob.glob(f"{data_dir}/simplii-chequing-data*.csv")):
        with open(path, newline="") as f:
            for r in csv.DictReader(f):
                rows.append({
                    "date": r["date"],
                    "description": r["description"],
                    "funds_out": _parse_float(r["funds_out"]),
                    "funds_in": _parse_float(r["funds_in"]),
                    "balance": _parse_float(r["balance"]),
                })
    rows = _dedup(rows, ["date", "description", "funds_out", "funds_in", "balance"])
    rows.sort(key=lambda r: r["date"])
    return rows


def load_visa(data_dir: str) -> list[dict]:
    """Load all Visa CSVs, parse amounts, deduplicate."""
    rows = []
    for path in sorted(glob.glob(f"{data_dir}/simplii-visa-data*.csv")):
        with open(path, newline="") as f:
            for r in csv.DictReader(f):
                rows.append({
                    "date": r["date"],
                    "post_date": r["post_date"],
                    "description": r["description"],
                    "amount": _parse_float(r["amount"]),
                    "category": r.get("category", ""),
                    "type": r.get("type", "purchase"),
                })
    rows = _dedup(rows, ["date", "post_date", "description", "amount"])
    rows.sort(key=lambda r: r["date"])
    return rows
