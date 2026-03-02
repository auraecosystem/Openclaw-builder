#!/usr/bin/env python3
"""Inject JSON data, CSS, and JS into HTML dashboard template.

Produces a single self-contained HTML file with everything inlined.
"""

import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description="Generate finance dashboard HTML")
    parser.add_argument("--input", required=True, help="JSON summary from analyze.py")
    parser.add_argument("--template", required=True, help="HTML template path")
    parser.add_argument("--output", required=True, help="Output HTML path")
    args = parser.parse_args()

    template_dir = Path(args.template).parent

    with open(args.input) as f:
        data = json.load(f)

    with open(args.template) as f:
        html = f.read()

    with open(template_dir / "styles.css") as f:
        css = f.read()

    with open(template_dir / "charts.js") as f:
        js = f.read()

    html = html.replace("<!-- DATA_PLACEHOLDER -->", f"<script>const DATA = {json.dumps(data)};</script>")
    html = html.replace("<!-- STYLES_PLACEHOLDER -->", css)
    html = html.replace("<!-- CHARTS_PLACEHOLDER -->", js)

    with open(args.output, "w") as f:
        f.write(html)

    print(args.output)


if __name__ == "__main__":
    main()
