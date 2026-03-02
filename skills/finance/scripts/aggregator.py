"""Monthly and category-level aggregation of transaction data."""

from collections import defaultdict


def monthly_cashflow(chequing: list[dict], visa: list[dict]) -> list[dict]:
    """Compute income, expenses, net, savings rate, and balance per month."""
    months: dict[str, dict] = defaultdict(lambda: {
        "income": 0.0, "chq_expenses": 0.0, "cc_expenses": 0.0, "balance": 0.0,
    })

    for r in chequing:
        m = r["date"][:7]
        months[m]["income"] += r["funds_in"]
        months[m]["chq_expenses"] += r["funds_out"]
        if r["balance"]:
            months[m]["balance"] = r["balance"]

    for r in visa:
        if r["type"] == "purchase":
            m = r["date"][:7]
            months[m]["cc_expenses"] += r["amount"]

    result = []
    for month in sorted(months):
        d = months[month]
        income = d["income"]
        expenses = d["chq_expenses"] + d["cc_expenses"]
        net = income - expenses
        rate = net / income if income > 0 else 0.0
        result.append({
            "month": month,
            "income": round(income, 2),
            "expenses": round(expenses, 2),
            "net": round(net, 2),
            "savings_rate": round(rate, 4),
            "balance": round(d["balance"], 2),
        })
    return result


def category_spending(visa: list[dict]) -> dict:
    """Total, average monthly, and count per CC category."""
    cats: dict[str, dict] = defaultdict(lambda: {"total": 0.0, "count": 0, "months": set()})

    for r in visa:
        if r["type"] != "purchase" or not r["category"]:
            continue
        c = cats[r["category"]]
        c["total"] += r["amount"]
        c["count"] += 1
        c["months"].add(r["date"][:7])

    result = {}
    for name, c in sorted(cats.items(), key=lambda x: -x[1]["total"]):
        n_months = max(len(c["months"]), 1)
        result[name] = {
            "total": round(c["total"], 2),
            "avg_monthly": round(c["total"] / n_months, 2),
            "count": c["count"],
        }
    return result


def monthly_by_category(visa: list[dict]) -> list[dict]:
    """Per-month spending broken down by CC category."""
    grid: dict[str, dict[str, float]] = defaultdict(lambda: defaultdict(float))

    for r in visa:
        if r["type"] != "purchase" or not r["category"]:
            continue
        grid[r["date"][:7]][r["category"]] += r["amount"]

    result = []
    for month in sorted(grid):
        row = {"month": month}
        row.update({k: round(v, 2) for k, v in grid[month].items()})
        result.append(row)
    return result
