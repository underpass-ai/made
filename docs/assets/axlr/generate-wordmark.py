#!/usr/bin/env python3
"""Render the aXlr wordmark with the same ANSI Shadow cell geometry as MADE and KMP."""
from pathlib import Path

LETTERS = {
    "A": [" █████╗ ", "██╔══██╗", "███████║", "██╔══██║", "██║  ██║", "╚═╝  ╚═╝"],
    "X": ["██╗  ██╗", "╚██╗██╔╝", " ╚███╔╝ ", " ██╔██╗ ", "██╔╝ ██╗", "╚═╝  ╚═╝"],
    "L": ["██╗     ", "██║     ", "██║     ", "██║     ", "███████╗", "╚══════╝"],
    "R": ["██████╗ ", "██╔══██╗", "██████╔╝", "██╔══██╗", "██║  ██║", "╚═╝  ╚═╝"],
}
WORD = "AXLR"
# One Spectrum colour per letter, the same four MADE uses: red, yellow, green, cyan.
LETTER_COLORS = ["#f04445", "#ffda36", "#32cd76", "#42d4ed"]
CW, CH, X0, Y0 = 24, 36, 44, 20

# Box-drawing strokes as (dx, dy, w, h) inside a 24×36 cell, copied from the KMP/MADE marks.
STROKES = {
    "█": [(0, 0, 24, 36)],
    "╗": [(0, 14, 18, 4), (14, 14, 4, 22), (0, 22, 10, 4), (6, 22, 4, 14)],
    "╔": [(6, 14, 18, 4), (6, 14, 4, 22), (14, 22, 10, 4), (14, 22, 4, 14)],
    "║": [(6, 0, 4, 36), (14, 0, 4, 36)],
    "╝": [(14, 0, 4, 26), (0, 22, 18, 4), (6, 0, 4, 18), (0, 14, 10, 4)],
    "╚": [(6, 0, 4, 26), (6, 22, 18, 4), (14, 0, 4, 18), (14, 14, 10, 4)],
    "═": [(0, 14, 24, 4), (0, 22, 24, 4)],
}

rows = ["".join(LETTERS[c][r] for c in WORD) for r in range(6)]
root = Path(__file__).resolve().parent
root.joinpath("axlr-wordmark.txt").write_text("\n".join(r.rstrip() for r in rows) + "\n")

width = X0 * 2 + len(rows[0]) * CW
height = Y0 * 2 + 6 * CH
svg = [
    f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title description">',
    '  <title id="title">aXlr</title>',
    '  <desc id="description">The aXlr mark in ANSI Shadow pixel letters, red, yellow, green and cyan like the ZX Spectrum stripes. By Underpass.</desc>',
    '  <g shape-rendering="crispEdges">',
]
for i, color in enumerate(LETTER_COLORS):
    d = "".join(
        f"M{X0 + (i * 8 + c) * CW + dx} {Y0 + r * CH + dy}h{w}v{h}h-{w}z"
        for r in range(6)
        for c, ch in enumerate(LETTERS[WORD[i]][r])
        for dx, dy, w, h in STROKES.get(ch, [])
    )
    svg.append(f'    <path fill="{color}" d="{d}"/>')
svg += ["  </g>", "</svg>"]
root.joinpath("axlr-wordmark.svg").write_text("\n".join(svg) + "\n")
