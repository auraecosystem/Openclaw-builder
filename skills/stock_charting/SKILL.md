---
name: stock-charting
description: >
  Render stock charts, candlestick charts, equity curves, and indicator panels
  with Python tooling, then attach the rendered image back to chat. Use when
  asked about: matplotlib, mplfinance, candlesticks, OHLCV charts, support and
  resistance, trend lines, VWAP, moving averages, RSI, MACD, Bollinger Bands,
  volume panels, annotations, trade markers, or sharing a PNG/JPG chart into
  Discord from OpenClaw.
metadata: { "openclaw": { "emoji": "📊", "requires": { "bins": ["python3"] } } }
---

# stock-charting

Use this skill when the user wants a rendered chart image, not just numeric output.

Primary goals:

- render a clean chart locally with Python
- include requested overlays and indicators
- save the image to disk
- attach it back to chat correctly

## When to use

- "plot this as a chart"
- "render a candlestick chart"
- "draw support and resistance"
- "add trend lines / VWAP / RSI / MACD"
- "mark entries and exits on the chart"
- "send the chart as a PNG to Discord"

## Defaults

- Prefer Python chart generation over ASCII or text tables when the user asks for a chart.
- Prefer `matplotlib` for general plotting and custom overlays.
- Prefer `mplfinance` for candlesticks if available.
- Use PNG by default for charts.
- Default size: roughly `1400x900` or similar readable landscape output.
- Use a white or neutral background unless the user asks otherwise.
- Keep labels, legend, and axis formatting readable at Discord size.
- Do not send an unlabeled chart unless the user explicitly asks for a bare visual.
- Default to a publication-style chart, not a debug plot.

## Minimum chart standard

Unless the user explicitly asks for a rough sketch, the chart should include:

- a specific title
- x-axis label
- y-axis label
- readable tick marks
- units where applicable
- a legend if there is more than one plotted series or overlay
- date range or timeframe context

For trading charts, assume the viewer may only see the image in Discord at medium size. Optimize for quick visual interpretation, not notebook-style defaults.

## Axis selection and labeling

Choose axes based on the actual meaning of the data, not just the array shape.

X-axis defaults:

- time or date for price series, candlesticks, indicators, and equity curves
- bar index only when timestamps are unavailable
- scenario / parameter bucket for sensitivity tables or sweep plots
- strike / expiry / delta only when plotting options-specific structures

Y-axis defaults:

- `Price ($)` for equities, ETFs, futures approximated in dollar price terms, and most candlestick charts
- `PnL ($)` for cumulative profit and loss
- `Equity ($)` or `Portfolio Value ($)` for account/equity curves
- `Return (%)` for normalized performance or percentage-change charts
- `Volume` for volume panels
- indicator-specific labels like `RSI`, `MACD`, `ATR`, or `Z-Score`
- `Spread (bps)` or `Yield (%)` for rates / macro series when appropriate

Labeling rules:

- If currency is known, include it in the y-axis label.
- If percent is plotted, use `%` in the label and format ticks as percentages when practical.
- If the x-axis is time, make the timeframe obvious in the title or x-axis label.
- If the chart is intraday, prefer labels like `Time (ET)`.
- If the chart is daily or longer, prefer `Date`.
- If the series is normalized or rebased, say so explicitly in the title or subtitle-equivalent text.

## Title and context rules

Use a title that answers: what is this, over what period, and with what transformation?

Good examples:

- `AAPL 5m Candlestick Chart with VWAP and Volume`
- `NVDA Daily Chart (Jan 2025 to Mar 2026) with 20/50 EMA`
- `Strategy Equity Curve: RTH Momentum Scalper (PnL in USD)`
- `Sensitivity of Compound Projection by Daily Return Assumption`

If helpful, add a concise subtitle-like note in the message text accompanying the image:

- symbol
- timeframe
- date range
- indicators shown
- whether scale is linear or log

## Scale selection

Choose scale intentionally:

- use linear scale for most intraday charts, short swing charts, and normal candlestick review
- use log scale for long-horizon growth charts, multi-month breakouts, and exponential equity curves
- use percent-normalized plots when comparing multiple symbols with very different nominal prices

If log scale materially improves readability, use it and say so in the title or legend.

## Trading-chart defaults

For OHLC / candlestick charts:

- x-axis: time/date
- primary y-axis: price
- lower panel: volume when volume is relevant
- title should include symbol and timeframe
- if VWAP or moving averages are shown, name them in the title or legend

For equity curves / backtest charts:

- x-axis: date or trade number, whichever is more honest for the analysis
- y-axis: `Equity ($)` or `PnL ($)`
- mark start and end values when useful
- include drawdown panel or drawdown annotation if that is central to the point

For sensitivity / scenario charts:

- x-axis should be the scenario variable, not a generic index
- y-axis should be the outcome variable with units
- if comparing several scenarios, use a legend and distinct lines/colors

## Attachment rule

If the user wants the chart sent back to chat, do **not** use `read` to "attach" it.

Use the `message` tool with `path` / `filePath` / `media` so the file is actually uploaded to Discord.

Good pattern:

1. Render chart to a real file like `./out/chart.png`
2. Send it with the message tool

Bad pattern:

1. Render chart
2. `read` the PNG
3. tell the user it was attached

`read` lets the model inspect the image, but it is not the reliable outbound attachment path.

## Rendering workflow

1. Confirm the chart type:
   - line / equity curve
   - OHLC / candlestick
   - multi-panel indicator chart
2. Gather the data:
   - OHLCV for price charts
   - time series for equity curves or indicators
3. Build overlays:
   - horizontal support / resistance
   - diagonal trend lines
   - moving averages
   - VWAP
   - Bollinger Bands
   - trade markers, labels, annotations
4. Build extra panels if needed:
   - volume
   - RSI
   - MACD
   - custom signals
5. Save the chart to disk
6. If requested, upload it with the `message` tool

## Candlestick guidance

- Use candlesticks when OHLCV data exists.
- Include a volume panel when volume matters.
- For intraday charts, ensure timestamps are clear and not overly dense.
- Label the price axis and time axis explicitly.
- For support / resistance:
  - use horizontal lines for clean price levels
  - label only the most important levels
- For trend lines:
  - draw from obvious swing pivots
  - do not clutter the chart with too many lines
- For entries / exits:
  - mark buys and sells distinctly
  - annotate price and timestamp only if it stays readable

## Indicator guidance

Common overlays:

- `SMA`
- `EMA`
- `VWAP`
- `Bollinger Bands`

Common lower panels:

- `RSI`
- `MACD`
- volume

Defaults:

- keep indicator count small unless the user explicitly wants many
- avoid turning the chart into noise
- prefer 1 to 3 overlays and at most 2 lower panels for normal use
- label indicator panels directly so the viewer does not need to infer them

## Annotation and legend rules

- If there is only one obvious series and no overlays, a legend may be omitted.
- If there are multiple lines, overlays, or scenarios, include a legend.
- Label support/resistance lines only when they are analytically important.
- Label trade markers when reviewing a specific setup or trade.
- Avoid overlapping text; fewer annotations are better than unreadable annotations.
- Prefer endpoint labels or sparse callouts over cluttered per-point labels.

## Advanced annotation toolkit (required for trade review charts)

Use these primitives deliberately:

- `ax.annotate()` with `arrowprops` for callouts
- `ax.axvline()` and `ax.axhline()` for event/level guides
- `matplotlib.patches` (`Rectangle`, `Ellipse`, `FancyArrowPatch`, `ConnectionPatch`) for zone highlights and structure emphasis
- `matplotlib.patheffects` (`Stroke` + `Normal`) to keep text readable over candles
- `axvspan()` for session/regime shading

Coordinate usage:

- Event markers in data coordinates (`xycoords='data'`)
- Summary boxes in axes coordinates (`transform=ax.transAxes`)
- Text offsets via `textcoords='offset points'` to reduce overlap

When using `mplfinance`:

- call `mpf.plot(..., returnfig=True)`
- annotate the returned price axis (`axes[0]`)
- keep overlays in `mpf.make_addplot()` and event labels as direct matplotlib annotations

When to use PIL post-processing:

- rounded translucent callout boxes, watermarking, pixel-level arrows/circles, or text stroke control after chart render.
- Use `ImageDraw.Draw(im, "RGBA")` for alpha compositing on RGB images.

Minimum annotation package for setup audits:

1. explicit alert timestamp marker
2. entry/stop lines
3. first invalidation or stop-breach marker (if applicable)
4. MFE and MAE markers with R-multiple labels
5. compact stats summary box (entry, stop, risk, MFE/MAE, latest R)

## Preferred format for annotated trade-review charts

When a user wants a post-mortem or "show me how this actually played out" chart, prefer this layout by default unless they ask for something else:

- **Canvas:** `~1600x1000` landscape PNG
- **Top-left panel:** main intraday candlestick chart focused on the alert/setup day
- **Bottom-left panel:** volume for the same window
- **Top-right panel:** compressed follow-through view (for example next day or 2-day 5m chart)
- **Bottom-right panel:** text summary box with setup, given grade, outcome grade, execution notes, and key stats

Preferred review overlays:

- alert timestamp vertical line
- entry / actionable area horizontal line
- stop or fail-line proxy
- same-day MFE marker
- same-day close marker when relevant
- next-day high / close markers when follow-through matters

Preferred text summary contents:

- setup name / playbook
- given grade and retrospective outcome grade
- one-sentence explanation of the executable trigger
- "if played properly" notes
- entry area, stop proxy, MFE, MAE, same-day close, next-day close
- R-multiple estimate when a reasonable stop proxy exists
- short verdict in plain language

Styling preferences from recent successful use:

- use sparse but explicit markup; avoid indicator clutter unless it is essential
- favor candlesticks + horizontal/vertical guides over many oscillators
- use path effects or boxed annotations so labels stay readable over candles
- make the chart understandable without reading the whole thread first
- optimize for Discord viewing size: high contrast, larger labels, minimal legend noise

For multi-name audit batches:

- render one PNG per symbol rather than one giant collage
- keep the structure consistent across symbols so comparisons are easy
- use deterministic output paths like `./out/<symbol>_grade_review.png`

## Python patterns

General matplotlib chart:

```bash
python3 render_chart.py
```

Candlesticks with `mplfinance` if available:

```python
import mplfinance as mpf
mpf.plot(df, type="candle", volume=True, savefig="chart.png")
```

If `mplfinance` is unavailable:

- fall back to plain matplotlib
- or install/use another available local package only if appropriate for the environment

## Output rules

- Save to a deterministic path when possible:
  - `./out/chart.png`
  - `./out/<symbol>_<timeframe>.png`
- Mention what was plotted:
  - symbol
  - timeframe
  - date range
  - indicators / levels included
- If sending to Discord, upload the image instead of only describing it.

## Chart quality checklist

- title is specific
- timeframe is clear
- axes are legible
- axes are labeled with units
- legend is minimal
- colors are distinguishable
- overlays are intentional
- chart is not visually overloaded
- scale choice is intentional
- the chart can be understood without reading the full chat history

## Trading-specific suggestions

- Use log scale for long multi-month trend charts when appropriate.
- Use linear scale for most short-term intraday charts.
- For backtest / equity-curve plots, label starting value, ending value, and major drawdowns when helpful.
- For setup review charts, include only the lines and indicators needed to explain the trade.

## Useful local context

- `/Users/ad/work/trading-tools/packages/trade_tws/src/trade_tws/README.md`
- `/Users/ad/work/trading-tools/packages/trade_yahoo/src/trade_yahoo/README.md`
- `/Users/ad/work/trading-tools/packages/trade_yahoo/src/trade_yahoo/macro_dashboard.py`
- `/Users/ad/work/ai/openclaw/skills/stock_trading/references/ross-cameron-rules.md`
- `/Users/ad/work/ai/openclaw/skills/stock_charting/references/annotation-playbook.md`

## External references (authoritative docs)

- Matplotlib annotate API: <https://matplotlib.org/stable/api/_as_gen/matplotlib.pyplot.annotate.html>
- Matplotlib patches API: <https://matplotlib.org/stable/api/patches_api.html>
- Matplotlib path effects guide: <https://matplotlib.org/stable/users/explain/artists/patheffects_guide.html>
- mplfinance docs/repo: <https://github.com/matplotlib/mplfinance>
- Pillow ImageDraw reference: <https://pillow.readthedocs.io/en/stable/reference/ImageDraw.html>
- Pillow ImageFont reference: <https://pillow.readthedocs.io/en/stable/reference/ImageFont.html>
