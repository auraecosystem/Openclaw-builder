"""Month-over-month and year-over-year trend analysis."""

from collections import defaultdict


def mom_spending_change(cashflow: list[dict]) -> list[dict]:
    """Percentage change in expenses vs prior month."""
    changes = []
    for i in range(1, len(cashflow)):
        prev = cashflow[i - 1]["expenses"]
        curr = cashflow[i]["expenses"]
        pct = ((curr - prev) / prev * 100) if prev > 0 else 0.0
        changes.append({
            "month": cashflow[i]["month"],
            "change_pct": round(pct, 1),
        })
    return changes


def yoy_comparison(cashflow: list[dict]) -> list[dict]:
    """Total spending per calendar month across years."""
    grid: dict[str, dict[str, float]] = defaultdict(lambda: defaultdict(float))

    for m in cashflow:
        year = m["month"][:4]
        month_num = m["month"][5:7]
        grid[month_num][year] += m["expenses"]

    result = []
    for month_num in sorted(grid):
        row = {"month_num": month_num}
        row.update({yr: round(val, 2) for yr, val in sorted(grid[month_num].items())})
        result.append(row)
    return result
