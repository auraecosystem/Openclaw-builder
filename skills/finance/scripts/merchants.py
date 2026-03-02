"""Merchant name normalization and spend ranking."""

import re
from collections import defaultdict

# Canonical merchant names: pattern → display name
# Checked against real data — each entry merges known variants
_ALIASES = {
    r"TIM HORTONS": "Tim Hortons",
    r"MCDONALD": "McDonald's",
    r"STARBUCKS": "Starbucks",
    r"WALMART": "Walmart",
    r"COSTCO GAS": "Costco Gas",
    r"COSTCO": "Costco",
    r"SHOPPERS DRUG": "Shoppers Drug Mart",
    r"CIRCLE K|IRVING QPS": "Circle K / Irving",
    r"PETRO.CANADA": "Petro-Canada",
    r"ATLANTIC SUPERS": "Atlantic Superstore",
    r"HONDA CANADA FINANCE": "Honda Canada Finance",
    r"TELUS MOBILITY|TELUS MOBILITE": "Telus Mobility",
    r"MASTERCARD.+PC FINANCIAL": "PC Financial Mastercard",
    r"VISA.+SIMPLII": "Simplii Visa Payment",
    r"VISA.+ROYAL BANK": "RBC Visa Payment",
    r"NSLSC": "NSLSC (Student Loan)",
    r"PAYPAL": "PayPal",
    r"ABM INTERAC (WITHDRAWAL|CHARGE)": "ATM Withdrawal",
    r"ABM WITHDRAWAL": "ATM Withdrawal",
    r"POS PURCHASE APPLE": "Apple",
    r"POS PURCHASE AFFIRM": "Affirm",
    r"POS MERCHANDISE": "POS Merchandise",
    r"SKIPTHEDISHES": "SkipTheDishes",
    r"DOORDASH": "DoorDash",
    r"AMAZON": "Amazon",
    r"STEAMGAMES|STEAM PURCHASE": "Steam",
    r"CLAUDE\.AI|ANTHROPIC": "Claude AI (Anthropic)",
    r"CHATGPT|OPENAI": "ChatGPT (OpenAI)",
    r"UBER": "Uber",
    r"ALCOOL NB LIQUO": "NB Liquor",
    r"DOMINO.S PIZZA": "Domino's Pizza",
    r"SUBWAY": "Subway",
    r"HANWELL ROAD IRVING": "Hanwell Road Irving",
    r"HUNTER.S ALE HOUSE": "Hunter's Ale House",
    r"AVATAR": "Avatar",
    r"TRANSFER OUT": "Transfer Out",
}

# Province codes for stripping location suffixes
_PROVINCES = r"\b(ON|BC|AB|QC|MB|SK|NS|NB|PE|NL|NT|YT|NU|CA)\b"


def _normalize(desc: str) -> str:
    """Map a transaction description to a canonical merchant name."""
    d = desc.strip()
    upper = d.upper()

    # E-transfers handled separately — don't normalize here
    if "E-TRANSFER" in upper:
        return d

    # Check aliases first (most reliable)
    for pattern, canonical in _ALIASES.items():
        if re.search(pattern, upper):
            return canonical

    # Fallback: strip noise from the raw description
    cleaned = upper
    cleaned = re.sub(r"\s*#\d+.*", "", cleaned)       # Store numbers
    cleaned = re.sub(r"\s+\d{5,}.*", "", cleaned)     # Long reference numbers
    cleaned = re.sub(rf"\s+{_PROVINCES}\s*$", "", cleaned)  # Trailing province
    cleaned = re.sub(r"\s+\d{3}-\d{3}-\d{4}.*", "", cleaned)  # Phone numbers
    cleaned = re.sub(r"\s{2,}", " ", cleaned).strip()

    # Titlecase the result for readability
    return cleaned.title() if cleaned else d


def top_merchants(chequing: list[dict], visa: list[dict], limit: int = 20) -> list[dict]:
    """Rank merchants by total spend across both accounts (excluding e-transfers)."""
    agg: dict[str, dict] = defaultdict(lambda: {"total": 0.0, "count": 0})

    for r in chequing:
        if r["funds_out"] > 0 and "E-TRANSFER" not in r["description"].upper():
            name = _normalize(r["description"])
            if name:
                agg[name]["total"] += r["funds_out"]
                agg[name]["count"] += 1

    for r in visa:
        if r["type"] == "purchase":
            name = _normalize(r["description"])
            if name:
                agg[name]["total"] += r["amount"]
                agg[name]["count"] += 1

    ranked = sorted(agg.items(), key=lambda x: -x[1]["total"])[:limit]
    return [
        {
            "description": name,
            "total_spent": round(d["total"], 2),
            "count": d["count"],
        }
        for name, d in ranked
    ]


def etransfer_recipients(chequing: list[dict]) -> list[dict]:
    """Break down e-transfer sends by recipient."""
    recipients: dict[str, dict] = defaultdict(
        lambda: {"total": 0.0, "count": 0, "last_date": "", "last_amount": 0.0}
    )

    for r in chequing:
        desc = r["description"]
        if "E-TRANSFER SEND" not in desc.upper() or r["funds_out"] <= 0:
            continue

        name = re.sub(r"(?i)INTERAC E-TRANSFER SEND\s*", "", desc).strip()
        if not name:
            name = "(unnamed)"

        d = recipients[name]
        d["total"] += r["funds_out"]
        d["count"] += 1
        if r["date"] > d["last_date"]:
            d["last_date"] = r["date"]
            d["last_amount"] = r["funds_out"]

    return [
        {
            "recipient": name,
            "total_sent": round(d["total"], 2),
            "count": d["count"],
            "last_date": d["last_date"],
            "last_amount": round(d["last_amount"], 2),
        }
        for name, d in sorted(recipients.items(), key=lambda x: -x[1]["total"])
    ]
