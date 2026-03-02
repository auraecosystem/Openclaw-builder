# Infrastructure and Execution Stack for Algo Trading

## The Minimum Viable Stack

You need exactly five things before you can run a live algo strategy:

1. **Clean, bias-free historical data** — the foundation everything else depends on
2. **A backtesting engine** — to validate ideas before capital is at risk
3. **Validation framework** — walk-forward, Monte Carlo, out-of-sample
4. **Risk management system** — hard-coded position limits, drawdown stops
5. **Live execution layer** — broker API, order management, monitoring

Most retail traders under-invest in (1) and (5) while over-investing in signal development. This is backwards.

---

## Data: The Non-Negotiable Foundation

### Equity Data Sources

| Provider | Data Type | Price | Notes |
|----------|-----------|-------|-------|
| **Norgate Data** | US/AU equities, futures, point-in-time | ~$50/mo | Best retail point-in-time data; survivorship-bias-free |
| **Polygon.io** | US equities, crypto, options | Free tier + $29+/mo | Good for recent real-time; historical quality varies |
| **Compustat** (Wharton WRDS) | Academic-grade fundamentals | Academic access | Institutional-quality; survivorship-bias-free |
| **Yahoo Finance / yfinance** | US/global equities | Free | Survivorship biased, errors in splits/dividends; not for production |
| **Alpha Vantage** | Global equities, forex | Free tier | Decent for prototyping, not production |
| **Quandl / Nasdaq Data Link** | Futures, alternative data | Varies | Good for futures and alternative datasets |

**Critical**: Yahoo Finance data has documented dividend adjustment errors, split errors, and survivorship bias. Never use it for production backtesting. It is acceptable only for rapid prototyping where you're testing concepts, not calibrating performance.

### Futures Data
- **Continuous contracts**: futures contracts expire and roll; you need a continuous price series. Options: back-adjusted (Panama method), ratio-adjusted, or raw with explicit roll handling
- **CQG, Rithmic, Interactive Brokers**: real-time and historical futures data
- **CSI Data**: professional-grade continuous futures with multiple roll methods

### Crypto Data
- **Binance Vision** (data.binance.vision): free historical OHLCV, 1m to daily
- **CryptoCompare, Kaiko**: more complete historical order book data
- Gotcha: timestamp format inconsistency in Binance historical data (ms vs. µs across different time periods — see algotrader project notes)

### Point-in-Time Requirement
For any strategy using fundamental data (P/E ratios, earnings, balance sheet), you **must** use point-in-time data that reflects what was actually known at each historical date. Using "as-of-today" fundamental data introduces look-ahead bias of 30-90 days (the typical earnings reporting lag).

---

## Backtesting Engines

### Event-Driven vs. Vectorized

**Vectorized backtesting** (pandas, numpy array operations):
- Fast to develop and run
- Cannot accurately model order types, fill logic, or intraday behavior
- Fine for daily strategies with simple fill assumptions (next open after close signal)
- Not suitable for intraday strategies or realistic execution modeling

**Event-driven backtesting** (simulates real-time order flow):
- More complex to build
- Handles order types (limit, stop, market), partial fills, latency simulation
- Required for intraday strategies and realistic performance estimation

### Platforms

| Platform | Type | Language | Notes |
|----------|------|----------|-------|
| **QuantConnect (Lean)** | Event-driven | C#/Python | Cloud-hosted; institutional-quality; free tier available |
| **Backtrader** | Event-driven | Python | Open-source; widely used; somewhat dated |
| **QSTrader** (QuantStart) | Event-driven | Python | Academic/tutorial; good for learning |
| **Zipline / Zipline-reloaded** | Event-driven | Python | Pandas-integrated; outdated main branch |
| **VectorBT** | Vectorized | Python | Extremely fast; good for initial exploration |
| **pysystemtrade** (Carver) | Systematic | Python | Futures/systematic trading; Robert Carver's framework |
| **NautilusTrader** | Event-driven | Python/Rust | High-performance; modern; production-grade |

**Recommendation**: use a vectorized engine (VectorBT or simple pandas) for initial exploration and idea screening. Switch to an event-driven engine (QuantConnect Lean or NautilusTrader) for final validation before live deployment.

---

## Brokers and Execution APIs

### For Retail Systematic Traders

| Broker | Asset Classes | API | Notes |
|--------|--------------|-----|-------|
| **Interactive Brokers (IBKR)** | Equities, futures, options, forex, bonds | REST + TWS API | Best retail broker for systematic trading; global access; low commissions |
| **Alpaca Markets** | US equities, crypto | REST + WebSocket | Commission-free; clean API; designed for algorithmic traders |
| **TD Ameritrade / Schwab** | US equities, options | REST API | Good option; being integrated into Schwab |
| **Tradovate / NinjaTrader** | Futures | REST + WebSocket | Good for futures-focused strategies |
| **Rithmic** | Futures | Binary protocol | Fastest execution for futures; sub-ms latency; complex API |
| **Binance / Bybit** | Crypto | REST + WebSocket | Best crypto liquidity; comprehensive API |

**IBKR is the default choice** for retail systematic traders who want access to multiple asset classes. The API is complex but well-documented and powerful.

### API Architecture

Use **both** REST and WebSocket simultaneously:
- **WebSocket**: real-time market data, order book, fills, position updates
- **REST**: account management, historical data requests, periodic health checks

This hybrid approach minimizes latency for time-sensitive operations while maintaining reliability for account management.

### Latency Considerations

For daily swing strategies: latency is largely irrelevant. A 100ms vs. 10ms fill difference is trivial on a multi-day holding period.

For intraday strategies: latency starts to matter significantly. Options:
- **VPS near exchange**: standard VPS colocation (20-40ms) is sufficient for most intraday strategies
- **Co-location (rack space)**: sub-5ms; required for HFT-adjacent strategies
- **FPGA/custom hardware**: sub-millisecond; HFT territory

Most retail algo strategies fall in the "VPS near exchange is sufficient" category.

---

## The Paper Trading Protocol

Paper trading is not "simulating in a spreadsheet." It means connecting to a live market data feed and executing real orders through a broker's paper/simulated account system, in real-time, during market hours.

### Why Paper Trading Matters

1. **Execution reality check**: your backtest assumes fills at signal prices. Paper trading reveals actual fill prices under live conditions
2. **Latency discovery**: how long does it actually take from signal to fill?
3. **Bug discovery**: order management bugs that never appeared in backtesting emerge under live conditions
4. **Psychology calibration**: experience the emotional texture of watching a live strategy before real money is at risk

### Duration

Minimum **30 trading days** (6 weeks). Ideally 60 days.

The strategy needs to experience:
- At least one choppy/trending regime transition
- At least one high-volatility day (VIX spike or major news event)
- At least one losing streak to test the drawdown logic

### Pass Criteria for Moving to Live

After paper trading:
- Actual fill prices are within 0.2% of backtested fill assumptions (for daily strategies)
- No critical bugs in order management or position tracking
- Maximum adverse excursion on individual trades is consistent with backtest expectations
- The live equity curve is within ±30% of the expected performance range (from walk-forward efficiency ratio)

---

## Monitoring and Alerting in Production

### What to Log (Everything)

```
For every order:
  - Signal timestamp (when decision was made)
  - Order submission timestamp
  - Fill timestamp
  - Expected fill price (from backtest model)
  - Actual fill price
  - Slippage (difference)
  - Commission paid

For every day:
  - Open positions and their P&L
  - Portfolio-level metrics (volatility, drawdown from peak, Sharpe trailing 60 days)
  - Any API errors or missed orders
```

Immutable logs — write-once, never modified. This allows forensic analysis when something goes wrong.

### Monitoring Checklist

| Alert | Threshold | Action |
|-------|-----------|--------|
| Slippage anomaly | Single trade >3x expected slippage | Investigate execution |
| Position size error | Position >110% of expected size | Immediate review |
| API timeout | Any execution API timeout | Check broker status; halt if repeated |
| Daily loss limit | Daily P&L < -2% | Stop trading for the day |
| Strategy drawdown | >15% from peak | Pause, investigate |
| Correlation spike | Two strategies >0.7 correlated | Reduce combined position |

### Server Infrastructure

- **VPS**: minimum 2 vCPUs, 4GB RAM, SSD storage
- **Location**: same datacenter or closest available to your primary exchange/broker
- **Uptime SLA**: 99.9% minimum; production strategies need reliability
- **Backup**: process manager (PM2, systemd) that auto-restarts on crash
- **Alerts**: PagerDuty, Telegram bot, or simple email for critical failures

---

## The Go-Live Checklist

```
PRE-LAUNCH:
[ ] Strategy has passed walk-forward with WFE > 0.5
[ ] Monte Carlo: positive expectancy in 80%+ of permutations
[ ] Transaction costs modeled explicitly; strategy still viable after costs
[ ] Paper traded for 30+ days; fill quality acceptable
[ ] All drawdown limits and kill switches coded and tested
[ ] Logging infrastructure active and tested
[ ] Broker API connection tested under load
[ ] Error handling for: API timeout, partial fill, connection drop

LAUNCH:
[ ] Start at 10-25% of intended capital
[ ] Monitor first 5 trades manually; verify fill prices
[ ] Confirm position sizes match expected sizes
[ ] Confirm P&L tracking matches broker P&L

POST-LAUNCH (60-90 days):
[ ] Live Sharpe within expected range (WFE-adjusted estimate)
[ ] Fill slippage within modeled assumption ±50%
[ ] No systematic bugs discovered
[ ] Psychology tested: experienced a drawdown and did not override system
[ ] Scale to full capital if above criteria met
```

---

## Sources

- [Algo Trading Infrastructure Guide — TradingFXVPS](https://tradingfxvps.com/api-trading-vps-optimization-2025-websocket-rest-for-algo-strategies/)
- [How to Forward Test a Trading Algo Before Going Live — QuantVPS](https://www.quantvps.com/blog/how-to-forward-test-a-trading-algo)
- [Retail Algorithmic Trading: A Complete Guide — IBKR Quant](https://www.interactivebrokers.com/campus/ibkr-quant-news/retail-algorithmic-trading-a-complete-guide/)
- [Top Futures Trading APIs — QuantVPS](https://www.quantvps.com/blog/top-futures-trading-apis-for-algo-traders)
- [Best Brokers for Algorithmic Trading — BrokerChooser](https://brokerchooser.com/best-brokers/best-brokers-for-algo-trading-in-the-united-states)
- [Successful Algorithmic Trading — QuantStart](https://www.quantstart.com/successful-algorithmic-trading-ebook/)
- [Data, APIs & Platform Infrastructure — SkillUpExchange](https://skillupexchange.com/data-apis-platform-infrastructure-algo-trading/)
