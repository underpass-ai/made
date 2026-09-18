#!/usr/bin/env python3
"""Render MADE's text and SVG wordmarks from one 5×7 pixel alphabet."""
from pathlib import Path

GLYPHS = (
    ("10001", "11011", "10101", "10101", "10001", "10001", "10001"),
    ("01110", "10001", "10001", "11111", "10001", "10001", "10001"),
    ("11110", "10001", "10001", "10001", "10001", "10001", "11110"),
    ("11111", "10000", "10000", "11110", "10000", "10000", "11111"),
)
ROWS = ["00".join(glyph[row] for glyph in GLYPHS) for row in range(7)]
ROOT = Path(__file__).resolve().parent
ROOT.joinpath("made-wordmark.txt").write_text(
    "\n".join("".join("##" if pixel == "1" else "  " for pixel in row).rstrip() for row in ROWS) + "\n"
)
ROOT.joinpath("made-wordmark-block.txt").write_text(
    "\n".join("".join("██" if pixel == "1" else "  " for pixel in row).rstrip() for row in ROWS) + "\n"
)
svg = [
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 680 240" role="img" aria-labelledby="title desc">',
    '<title id="title">MADE</title>',
    '<desc id="desc">MADE in white pixel letters on dark ink, with red, yellow, green and cyan Spectrum-inspired stripes.</desc>',
    '<rect width="680" height="240" rx="12" fill="#101319"/>',
    '<g fill="#f7f8fa" shape-rendering="crispEdges">',
]
for y,row in enumerate(ROWS):
    for x,pixel in enumerate(row):
        if pixel == "1":
            svg.append(f'<rect x="{54+x*22}" y="{35+y*22}" width="22" height="22"/>')
svg.append('</g>')
for i,color in enumerate(("#f04445", "#ffd43b", "#37c978", "#42cfee")):
    x=492+i*27
    svg.append(f'<path d="M{x} 219 l25 -25 h27 l-25 25z" fill="{color}"/>')
svg.append('</svg>')
ROOT.joinpath("made-wordmark.svg").write_text("\n".join(svg)+"\n")
