"""
Portfolio game state and report generation.

Used in two ways:
1. Aggregating vectorbt backtest results into a unified report
2. Tracking a live paper-trading game state (persistent JSON)
"""

import json
import math
from dataclasses import dataclass, field, asdict
from datetime import datetime
from pathlib import Path


@dataclass
class Position:
    ticker: str
    setup: str              # 'breakout' | 'ep' | 'parabolic'
    entry_date: str
    entry_price: float
    shares: int
    stop_price: float
    direction: str          # 'long' | 'short'
    exit_date: str | None = None
    exit_price: float | None = None
    pnl: float = 0.0
    pnl_pct: float = 0.0
    hold_days: int = 0
    exit_reason: str = ""   # 'trailing_sma' | 'stop' | 'partial' | 'forced'


@dataclass
class PortfolioState:
    init_cash: float = 100_000.0
    cash: float = 100_000.0
    equity: float = 100_000.0
    peak_equity: float = 100_000.0
    max_drawdown: float = 0.0
    open_positions: list[Position] = field(default_factory=list)
    closed_positions: list[Position] = field(default_factory=list)
    equity_curve: list[dict] = field(default_factory=list)

    def open_position_count(self) -> int:
        return len(self.open_positions)

    def add_position(self, pos: Position) -> None:
        cost = pos.entry_price * pos.shares
        self.cash -= cost
        self.open_positions.append(pos)

    def close_position(
        self,
        ticker: str,
        exit_date: str,
        exit_price: float,
        reason: str,
    ) -> Position | None:
        for i, pos in enumerate(self.open_positions):
            if pos.ticker == ticker:
                pos.exit_date = exit_date
                pos.exit_price = exit_price
                pos.exit_reason = reason
                if pos.direction == "long":
                    pos.pnl = (exit_price - pos.entry_price) * pos.shares
                else:
                    pos.pnl = (pos.entry_price - exit_price) * pos.shares
                pos.pnl_pct = pos.pnl / (pos.entry_price * pos.shares)
                # Parse dates robustly
                try:
                    d1 = datetime.fromisoformat(pos.entry_date)
                    d2 = datetime.fromisoformat(exit_date)
                    pos.hold_days = (d2 - d1).days
                except ValueError:
                    pos.hold_days = 0
                self.cash += exit_price * pos.shares
                self.closed_positions.append(pos)
                self.open_positions.pop(i)
                return pos
        return None

    def update_equity(self, date: str, prices: dict[str, float]) -> None:
        unrealized = sum(
            (prices.get(p.ticker, p.entry_price) - p.entry_price) * p.shares
            if p.direction == "long"
            else (p.entry_price - prices.get(p.ticker, p.entry_price)) * p.shares
            for p in self.open_positions
        )
        self.equity = self.cash + unrealized
        if self.equity > self.peak_equity:
            self.peak_equity = self.equity
        dd = (self.peak_equity - self.equity) / self.peak_equity
        if dd > self.max_drawdown:
            self.max_drawdown = dd
        self.equity_curve.append({"date": date, "equity": round(self.equity, 2)})

    def summary(self) -> dict:
        return {
            "cash": round(self.cash, 2),
            "equity": round(self.equity, 2),
            "peak_equity": round(self.peak_equity, 2),
            "max_drawdown": round(self.max_drawdown, 4),
            "open_positions": len(self.open_positions),
            "closed_positions": len(self.closed_positions),
        }


def _safe_div(a: float, b: float, default: float = 0.0) -> float:
    return a / b if b != 0 else default


def _equity_curve_from_trades(
    trades: list,
    init_cash: float,
) -> list[dict]:
    """Build a per-trade equity curve when no daily curve is available.

    Sorts trades by exit date, accumulates running equity. Each point represents
    the portfolio value immediately after that trade closes.
    """
    closed = sorted(
        [t for t in trades if t.exit_date],
        key=lambda t: str(t.exit_date),
    )
    equity = init_cash
    curve = [{"date": closed[0].entry_date, "equity": init_cash}] if closed else []
    for t in closed:
        equity += t.pnl
        curve.append({"date": str(t.exit_date), "equity": round(equity, 2)})
    return curve


def generate_report(state: PortfolioState) -> dict:
    """Produce a full backtest/game report from a PortfolioState."""
    trades = state.closed_positions
    n = len(trades)
    if n == 0:
        return {"error": "no closed trades", "summary": state.summary()}

    wins = [t for t in trades if t.pnl > 0]
    losses = [t for t in trades if t.pnl <= 0]
    win_rate = len(wins) / n

    gross_profit = sum(t.pnl for t in wins)
    gross_loss = abs(sum(t.pnl for t in losses))
    profit_factor = _safe_div(gross_profit, gross_loss)

    avg_win = _safe_div(gross_profit, len(wins)) if wins else 0.0
    avg_loss = _safe_div(gross_loss, len(losses)) if losses else 0.0
    avg_hold = sum(t.hold_days for t in trades) / n

    # Use equity_curve from state if available; build from trades otherwise.
    equity_curve = state.equity_curve
    if not equity_curve:
        equity_curve = _equity_curve_from_trades(trades, state.init_cash)

    total_pnl = sum(t.pnl for t in trades)
    final_equity = state.init_cash + total_pnl
    total_return = total_pnl / state.init_cash

    # CAGR: span from first entry to last exit
    cagr = 0.0
    if len(equity_curve) >= 2:
        try:
            d_start = datetime.fromisoformat(str(equity_curve[0]["date"]))
            d_end = datetime.fromisoformat(str(equity_curve[-1]["date"]))
            years = (d_end - d_start).days / 365.25
            if years > 0 and final_equity > 0:
                cagr = (final_equity / state.init_cash) ** (1 / years) - 1
        except (ValueError, KeyError):
            pass

    # Max drawdown from cumulative P&L
    max_drawdown = 0.0
    if equity_curve:
        peak = equity_curve[0]["equity"]
        for point in equity_curve:
            eq = point["equity"]
            if eq > peak:
                peak = eq
            if peak > 0:
                dd = (peak - eq) / peak
                if dd > max_drawdown:
                    max_drawdown = dd

    # Sharpe/Sortino from equity curve returns
    sharpe = sortino = 0.0
    if len(equity_curve) > 30:
        eq_values = [p["equity"] for p in equity_curve]
        daily_rets = [
            (eq_values[i] - eq_values[i - 1]) / eq_values[i - 1]
            for i in range(1, len(eq_values))
        ]
        mean_ret = sum(daily_rets) / len(daily_rets)
        variance = sum((r - mean_ret) ** 2 for r in daily_rets) / len(daily_rets)
        std_ret = math.sqrt(variance)
        if std_ret > 0:
            sharpe = (mean_ret / std_ret) * math.sqrt(252)
        downside = [r for r in daily_rets if r < 0]
        if downside:
            down_var = sum(r ** 2 for r in downside) / len(downside)
            down_std = math.sqrt(down_var)
            if down_std > 0:
                sortino = (mean_ret / down_std) * math.sqrt(252)

    # Per-setup breakdown (dynamic — handles breakout_quick, breakout_runner, etc.)
    setups = {}
    setup_names = sorted(set(t.setup for t in trades))
    for setup_name in setup_names:
        subset = [t for t in trades if t.setup == setup_name]
        if subset:
            subset_wins = [t for t in subset if t.pnl > 0]
            setups[setup_name] = {
                "trades": len(subset),
                "win_rate": round(len(subset_wins) / len(subset), 3),
                "total_pnl": round(sum(t.pnl for t in subset), 2),
                "avg_hold_days": round(sum(t.hold_days for t in subset) / len(subset), 1),
            }

    # Monthly returns
    monthly: dict[str, float] = {}
    if state.equity_curve:
        prev_val = state.init_cash
        prev_month = state.equity_curve[0]["date"][:7]
        for point in state.equity_curve:
            month = point["date"][:7]
            if month != prev_month:
                monthly[prev_month] = round((prev_val - state.init_cash) / state.init_cash, 4)
                prev_month = month
            prev_val = point["equity"]

    # Best/worst trades
    sorted_trades = sorted(trades, key=lambda t: t.pnl, reverse=True)
    best = [asdict(t) for t in sorted_trades[:10]]
    worst = [asdict(t) for t in sorted_trades[-10:]]

    return {
        "generated_at": datetime.now().isoformat(),
        "total_trades": n,
        "win_rate": round(win_rate, 3),
        "profit_factor": round(profit_factor, 2),
        "avg_win": round(avg_win, 2),
        "avg_loss": round(avg_loss, 2),
        "avg_hold_days": round(avg_hold, 1),
        "total_return": round(total_return, 4),
        "cagr": round(cagr, 4),
        "max_drawdown": round(max_drawdown, 4),
        "sharpe": round(sharpe, 2),
        "sortino": round(sortino, 2),
        "init_cash": state.init_cash,
        "final_equity": round(final_equity, 2),
        "per_setup": setups,
        "best_trades": best,
        "worst_trades": worst,
        "monthly_returns": monthly,
        "equity_curve": equity_curve,
        "summary": state.summary(),
    }


def save_report(report: dict, output_path: str) -> None:
    """Write report JSON to output_path."""
    Path(output_path).parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(report, f, indent=2, default=str)


def load_state(path: str) -> PortfolioState:
    """Load persistent game state from JSON file."""
    with open(path) as f:
        data = json.load(f)
    state = PortfolioState(
        init_cash=data.get("init_cash", 100_000.0),
        cash=data.get("cash", 100_000.0),
        equity=data.get("equity", 100_000.0),
        peak_equity=data.get("peak_equity", 100_000.0),
        max_drawdown=data.get("max_drawdown", 0.0),
        equity_curve=data.get("equity_curve", []),
    )
    state.open_positions = [Position(**p) for p in data.get("open_positions", [])]
    state.closed_positions = [Position(**p) for p in data.get("closed_positions", [])]
    return state


def save_state(state: PortfolioState, path: str) -> None:
    """Persist game state to JSON file."""
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        json.dump(asdict(state), f, indent=2, default=str)
