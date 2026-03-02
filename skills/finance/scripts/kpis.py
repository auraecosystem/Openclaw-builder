"""Key performance indicator calculations."""


def compute_kpis(cashflow: list[dict]) -> dict:
    """Derive headline KPIs from monthly cashflow data."""
    if not cashflow:
        return {}

    total_income = sum(m["income"] for m in cashflow)
    total_expenses = sum(m["expenses"] for m in cashflow)
    overall_rate = (total_income - total_expenses) / total_income if total_income > 0 else 0.0

    # Average monthly spending over last 6 months
    recent = cashflow[-6:]
    avg_monthly = sum(m["expenses"] for m in recent) / len(recent) if recent else 0.0

    # Latest balance from most recent month with a balance
    latest_balance = 0.0
    for m in reversed(cashflow):
        if m["balance"]:
            latest_balance = m["balance"]
            break

    emergency_months = latest_balance / avg_monthly if avg_monthly > 0 else 0.0

    return {
        "total_income": round(total_income, 2),
        "total_expenses": round(total_expenses, 2),
        "overall_savings_rate": round(overall_rate, 4),
        "avg_monthly_spending": round(avg_monthly, 2),
        "latest_balance": round(latest_balance, 2),
        "emergency_fund_months": round(emergency_months, 1),
    }
