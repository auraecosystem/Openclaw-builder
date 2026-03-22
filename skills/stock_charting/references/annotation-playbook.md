# Stock Chart Annotation Playbook (Matplotlib + mplfinance + PIL)

Purpose: make chart callouts readable and decision-useful in Discord-sized images.

## Core principles

1. Prefer **few, high-signal annotations** over dense labels.
2. Use **consistent color semantics**:
   - green = favorable / target / breakout success
   - red = risk / stop / invalidation
   - blue = reference lines (entry, VWAP)
   - purple = event markers (alert timestamps)
3. Keep annotation text short; move detail into one summary box.
4. Protect readability with text background boxes or stroke outlines.

## Matplotlib primitives to use

- `ax.annotate()` for text + arrow callouts.
- `matplotlib.patches` for highlighted areas:
  - `Rectangle` (zones, ranges)
  - `Ellipse` (focus regions)
  - `FancyArrowPatch` (custom arrows)
  - `ConnectionPatch` (link points between subplots)
- `ax.axhline()`, `ax.axvline()`, `ax.plot()` for levels and guides.
- `Artist.set_path_effects()` for text/line contrast (outline, shadow).

## Coordinate rules (important)

- Price-event labels: use data coordinates (`xycoords='data'`).
- Persistent summary text: use axes coordinates (`transform=ax.transAxes`) so box stays in top-left/top-right even on autoscale.
- Offset callout text with `textcoords='offset points'` to avoid overlap.

## Recommended annotation patterns

### 1) Event pin (alert/entry/invalidation)

Use a vertical line + anchored label:

- `ax.axvline(ts, linestyle=':', linewidth=1.2)`
- text box near top (`va='top'`) with timestamp + event name.

### 2) Entry/stop/target rails

- dashed horizontal lines (`axhline`) with concise labels at right edge.
- keep stop red, entry blue, target green.

### 3) MFE/MAE markers

- marker on highest high / lowest low post-entry.
- short label: `MFE +1.6R`, `MAE -1.3R`.

### 4) Regime or session shading

Use `axvspan(start,end,alpha=0.08)` or patch rectangles for:

- pre-alert context
- active-trade window
- post-invalidation drift

### 5) Price structure callouts

Use small arrows to identify:

- breakout attempt
- pullback breach
- reclaim/retest

## Text readability defaults

- Font sizes:
  - chart title: 12–14
  - axis labels: 10–11
  - callouts: 8–9
- Always provide either:
  - `bbox=dict(facecolor='white', alpha=0.7, edgecolor='gray')`, or
  - path effects outline:
    - `Stroke(linewidth=2-3, foreground='white') + Normal()`

## mplfinance integration pattern

Use:

- `mpf.plot(..., returnfig=True)` then annotate on returned axes.
- `mpf.make_addplot()` for overlays (EMA/VWAP/bands).
- patches/annotate calls directly on `axes[0]` (price panel).

Practical panel mapping:

- `axes[0]` price panel
- `axes[2]` volume panel (when `volume=True`)

## PIL post-processing use cases

Use PIL when Matplotlib text placement is done but you need final polish:

- translucent panel headers
- rounded rectangle labels
- branding/watermark
- final arrows/circles at pixel-level

Key PIL APIs:

- `ImageDraw.Draw(im, "RGBA")` for alpha blending on RGB base
- `draw.rounded_rectangle(...)`
- `draw.line(...)`, `draw.ellipse(...)`, `draw.polygon(...)`
- `draw.text(..., stroke_width=2, stroke_fill='white')` (for contrast)

## Clutter control checklist

Before exporting, remove any annotation that fails one of:

- changes a decision?
- highlights a non-obvious feature?
- remains readable at Discord preview size?

If not, delete it.

## Suggested default annotation set for trade-review charts

1. Alert timestamp marker
2. Entry + stop levels
3. First stop-breach marker (if occurred)
4. MFE and MAE points with R values
5. One summary stats box (entry, stop, risk, MFE/MAE, latest R)
6. Optional: AVWAP-from-alert line

## Sources consulted

- Matplotlib annotate API: https://matplotlib.org/stable/api/_as_gen/matplotlib.pyplot.annotate.html
- Matplotlib patches API: https://matplotlib.org/stable/api/patches_api.html
- Matplotlib path effects guide: https://matplotlib.org/stable/users/explain/artists/patheffects_guide.html
- mplfinance repository/docs: https://github.com/matplotlib/mplfinance
- Pillow ImageDraw docs: https://pillow.readthedocs.io/en/stable/reference/ImageDraw.html
- Pillow ImageFont docs: https://pillow.readthedocs.io/en/stable/reference/ImageFont.html
