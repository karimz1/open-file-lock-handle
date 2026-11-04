#!/usr/bin/env python3
"""Regenerate the NSIS installer bitmaps from the OFLH logo (app-icon.svg).

    python3 crates/oflh-desktop/windows/branding/generate.py [--font PATH]

Needs Pillow and a bold sans font (DejaVu Sans Bold by default). Writes
sidebar.bmp (492x942, welcome and finish pages) and header.bmp (450x171, other
pages) next to this script as 24-bit BMPs, the format NSIS expects.

The bitmaps are rendered at 300% resolution, including text and glyphs.
MUI2 FitControl scales them down to its DPI-scaled control, retaining detail
at 100%, 150%, 200% and 300% rather than enlarging a 96-DPI raster.
"""
import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
PRIMARY = (0x42, 0x64, 0xDB)  # --primary in ui/src/style.css and app-icon.svg
PRIMARY_DARK = (0x2B, 0x45, 0xA8)
WHITE = (255, 255, 255)
SS = 4  # supersampling factor
DPI = 3  # source pixels per logical 96-DPI pixel


def draw_glyph(draw, ox, oy, scale, colour, width=25):
    """The document-and-magnifier glyph of app-icon.svg (512 unit viewBox)."""
    def p(x, y):
        return (ox + x * scale, oy + y * scale)

    w = max(1, round(width * scale))
    for line in (
        [(158, 110), (288, 110), (355, 178), (355, 281)],
        [(158, 110), (158, 402), (272, 402)],
        [(288, 110), (288, 180), (355, 180)],
        [(350, 356), (399, 405)],
        [(210, 236), (245, 236)],
    ):
        pts = [p(*xy) for xy in line]
        draw.line(pts, fill=colour, width=w, joint="curve")
        for x, y in pts:
            draw.ellipse([x - w / 2, y - w / 2, x + w / 2, y + w / 2], fill=colour)
    cx, cy = p(305, 310)
    r = 61 * scale
    draw.ellipse([cx - r - w / 2, cy - r - w / 2, cx + r + w / 2, cy + r + w / 2], outline=colour, width=w)


def tile(size):
    """The full app icon: blue rounded square with the white glyph."""
    img = Image.new("RGBA", (size * SS, size * SS), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    s = size * SS / 512
    d.rounded_rectangle([16 * s, 16 * s, 496 * s, 496 * s], radius=108 * s, fill=PRIMARY)
    draw_glyph(d, 0, 0, s, WHITE)
    return img.resize((size, size), Image.LANCZOS)


def sidebar(font_path):
    w, h = 164 * SS, 314 * SS
    img = Image.new("RGB", (w, h))
    d = ImageDraw.Draw(img)
    for y in range(h):  # vertical brand gradient
        t = y / (h - 1)
        d.line([(0, y), (w, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(PRIMARY, PRIMARY_DARK)))
    # Large, faint glyph as a watermark in the lower half.
    wm = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    draw_glyph(ImageDraw.Draw(wm), -40 * SS, 150 * SS, 260 * SS / 512, (255, 255, 255, 28))
    img.paste(wm, (0, 0), wm)
    small = img.resize((164 * DPI, 314 * DPI), Image.LANCZOS)
    icon = tile(64 * DPI)
    white_tile = Image.new("RGBA", (72 * DPI, 72 * DPI), (0, 0, 0, 0))
    ImageDraw.Draw(white_tile).rounded_rectangle([0, 0, 71 * DPI, 71 * DPI], radius=17 * DPI, fill=WHITE)
    small.paste(white_tile, (46 * DPI, 36 * DPI), white_tile)
    small.paste(icon, (50 * DPI, 40 * DPI), icon)
    d = ImageDraw.Draw(small)
    title = ImageFont.truetype(font_path, 30 * DPI)
    sub = ImageFont.truetype(font_path, 11 * DPI)
    d.text((82 * DPI, 128 * DPI), "OFLH", font=title, fill=WHITE, anchor="mt")
    d.text((82 * DPI, 166 * DPI), "Open File Lock Handle", font=sub, fill=(220, 228, 250), anchor="mt")
    d.line([(62 * DPI, 186 * DPI), (102 * DPI, 186 * DPI)], fill=(160, 178, 238), width=DPI)
    return small


def header(font_path):
    img = Image.new("RGB", (150 * DPI, 57 * DPI), WHITE)  # MUI header background is white
    icon = tile(40 * DPI)
    img.paste(icon, (104 * DPI, 8 * DPI), icon)
    d = ImageDraw.Draw(img)
    d.text((98 * DPI, 19 * DPI), "OFLH", font=ImageFont.truetype(font_path, 17 * DPI), fill=PRIMARY, anchor="rt")
    return img


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--font", default="/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf")
    args = ap.parse_args()
    sidebar(args.font).save(HERE / "sidebar.bmp")
    header(args.font).save(HERE / "header.bmp")


if __name__ == "__main__":
    main()
