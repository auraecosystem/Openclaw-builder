#!/usr/bin/env python3
"""CLI entry point: load CSVs, run all analyses, write JSON summary."""

import argparse
import json
import os
import sys
from datetime import datetime

# Allow imports from scripts/ directory
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from loader import load_chequing, load_visa  # noqa: E402
from aggregator import monthly_cashflow, category_spending, monthly_by_category  # noqa: E402
from kpis import compute_kpis  # noqa: E402
from merchants import top_merchants, etransfer_recipients  # noqa: E402
from trends import mom_spending_change, yoy_comparison  # noqa: E402


def main():
    parser = argparse.ArgumentParser(description="Analyze Simplii Financial transaction data")
    parser.add_argument("--data-dir", required=True, help="Directory containing CSV files")
    parser.add_argument("--output", required=True, help="Output JSON file path")
    args = parser.parse_args()

    chq = load_chequing(args.data_dir)
    visa = load_visa(args.data_dir)

    cashflow = monthly_cashflow(chq, visa)

    all_dates = [r["date"] for r in chq] + [r["date"] for r in visa]

    summary = {
        "generated_at": datetime.now().isoformat(),
        "date_range": {
            "start": min(all_dates) if all_dates else "",
            "end": max(all_dates) if all_dates else "",
        },
        "kpis": compute_kpis(cashflow),
        "monthly_cashflow": cashflow,
        "category_spending": category_spending(visa),
        "monthly_by_category": monthly_by_category(visa),
        "top_merchants": top_merchants(chq, visa),
        "etransfers": etransfer_recipients(chq),
        "trends": {
            "mom_spending_change": mom_spending_change(cashflow),
            "yoy_comparison": yoy_comparison(cashflow),
        },
    }

    with open(args.output, "w") as f:
        json.dump(summary, f, indent=2)

    print(
        f"Analyzed {len(chq)} chequing + {len(visa)} visa txns "
        f"across {len(cashflow)} months → {args.output}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
