---
name: finance
description: >
  Personal financial advisor and analyst. Use when asked about: spending analysis,
  budgets, savings rate, Canadian taxes (TFSA/RRSP/FHSA), investment strategy,
  tax deductions, receipt tracking, wealth building, or to generate financial
  dashboards. Loads Simplii chequing + Visa CSV transaction data.
metadata:
  {
    "openclaw":
      {
        "emoji": "💰",
        "requires": { "bins": ["python3"] },
      },
  }
---

# Finance

Personal financial advisor with access to your Simplii Financial transaction history (chequing + Visa) and Canadian personal finance knowledge.

## When to use

Trigger on any of:
- "how much did I spend on..." / "what's my spending"
- "savings rate" / "budget" / "cash flow"
- "show my finances" / "generate a dashboard"
- "tax advice" / "what can I write off" / "TFSA or RRSP"
- "investment strategy" / "couch potato" / "ETF"
- "build wealth" / "financial plan"

## Data

Two CSV files in `{baseDir}/data/`:

**Chequing** (`simplii-chequing-data_2019-03_to_2026-01.csv`):
`date, description, funds_out, funds_in, balance`

**Visa** (`simplii-visa-data_2023-01_to_2026-02.csv`):
`date, post_date, description, amount, category, type`

CC categories: Restaurants, Retail and Grocery, Personal and Household Expenses, Foreign Currency Transactions, Professional and Financial Services, Home and Office Improvement, Health and Education, Transportation, Entertainment, Other Transactions.

CC types: purchase, payment, interest, fee.

## Quick analysis

Generate a JSON summary of all transaction data:

```bash
python3 {baseDir}/scripts/analyze.py --data-dir {baseDir}/data --output /tmp/finance-summary.json
```

Output includes: monthly cash flow, category spending, savings rate, top merchants, KPIs, and trends.

## Dashboard

Generate and deliver a financial dashboard as image(s) via the current messaging channel:

### Step 1: Generate HTML

```bash
python3 {baseDir}/scripts/dashboard.py \
  --input /tmp/finance-summary.json \
  --template {baseDir}/assets/dashboard-template.html \
  --output ~/clawd/canvas/finance-dashboard.html
```

### Step 2: Present on canvas

Find a connected node, then present:

```
canvas action:present node:<node-id> target:http://<canvas-host>:18793/__openclaw__/canvas/finance-dashboard.html
```

Use `openclaw nodes list` to find a node with canvas capability. The canvas host URL depends on `gateway.bind` setting (Tailscale hostname, LAN IP, or localhost).

### Step 3: Capture and send as image

Wait ~2 seconds for Chart.js to render, then snapshot:

```
canvas action:snapshot node:<node-id> outputFormat:png maxWidth:1200 delayMs:2000
```

The snapshot is automatically extracted from the tool result and sent as a Discord (or other channel) attachment. Include a brief text caption summarizing the KPIs.

### Step 4: Hide canvas

```
canvas action:hide node:<node-id>
```

## Answering questions

For spending/budget questions: run analyze.py, then answer from the JSON data.

For tax, investing, or wealth-building questions: read the relevant reference file:

| Topic | Reference |
|-------|-----------|
| Tax deductions & receipts | `{baseDir}/references/canadian-tax-deductions.md` |
| TFSA / RRSP / FHSA priority | `{baseDir}/references/tfsa-rrsp-fhsa-priority.md` |
| ETF investing (Couch Potato) | `{baseDir}/references/couch-potato-investing.md` |
| Budgeting (YNAB, 50/30/20) | `{baseDir}/references/budgeting-methodology.md` |
| Late-start wealth building | `{baseDir}/references/wealth-building-late-start.md` |
| Tax optimization strategies | `{baseDir}/references/tax-optimization-strategies.md` |

Read the reference first, then synthesize advice personalized to the user's actual spending data.
