#!/usr/bin/env python3
"""Generates assets/best_loupe.ico (a magnifying glass, drawn separately per size, supersampled so
small sizes stay crisp) and assets/best_loupe_paused.ico (the same, desaturated and faded: tray
icon while the lens is paused). Run from the repo root: python3 tools/make_icon.py [preview.png]"""
import io
import struct
import sys
from PIL import Image, ImageDraw

SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
SS = 8  # supersampling factor

RING = (38, 50, 66, 255)       # dark slate: readable on light taskbars
RIM = (255, 255, 255, 235)     # thin light rim: readable on dark taskbars
GLASS_TOP = (214, 236, 255, 255)
GLASS_BOTTOM = (112, 172, 238, 255)
SHINE = (255, 255, 255, 200)


def draw(size: int) -> Image.Image:
    s = size * SS
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    # Lens circle in the upper-left, handle towards the lower-right corner.
    ring_w = max(2.0, size * 0.105) * SS
    r = s * 0.335
    cx = cy = s * 0.405
    rim_w = max(1.0, size / 32) * SS

    # Handle (drawn first, under the ring): a thick rounded line at 45 degrees.
    hw = max(3.0, size * 0.17) * SS
    x0, y0 = cx + r * 0.72, cy + r * 0.72
    x1, y1 = s * 0.86, s * 0.86
    for w, col in ((hw + 2 * rim_w, RIM), (hw, RING)):
        d.line([(x0, y0), (x1, y1)], fill=col, width=int(w))
        d.ellipse([x1 - w / 2, y1 - w / 2, x1 + w / 2, y1 + w / 2], fill=col)

    # Glass: vertical gradient inside the ring.
    glass = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glass)
    top, bottom = cy - r, cy + r
    for y in range(int(top), int(bottom) + 1):
        t = (y - top) / max(1.0, bottom - top)
        col = tuple(int(GLASS_TOP[i] + (GLASS_BOTTOM[i] - GLASS_TOP[i]) * t) for i in range(4))
        gd.line([(0, y), (s, y)], fill=col)
    mask = Image.new("L", (s, s), 0)
    ImageDraw.Draw(mask).ellipse([cx - r, cy - r, cx + r, cy + r], fill=255)
    img.paste(glass, (0, 0), mask)

    # Shine arc (skipped at the tiniest sizes where it would be mush).
    if size >= 20:
        sr = r * 0.62
        shine = Image.new("RGBA", (s, s), (0, 0, 0, 0))
        ImageDraw.Draw(shine).arc([cx - sr, cy - sr, cx + sr, cy + sr], start=200, end=260, fill=SHINE,
                                  width=int(max(1.5, size * 0.06) * SS))
        img.alpha_composite(shine)
        d = ImageDraw.Draw(img)

    # Ring with outer and inner light rims.
    d.ellipse([cx - r - ring_w / 2 - rim_w, cy - r - ring_w / 2 - rim_w,
               cx + r + ring_w / 2 + rim_w, cy + r + ring_w / 2 + rim_w], outline=RIM, width=int(rim_w))
    d.ellipse([cx - r - ring_w / 2, cy - r - ring_w / 2, cx + r + ring_w / 2, cy + r + ring_w / 2],
              outline=RING, width=int(ring_w))
    return img.resize((size, size), Image.LANCZOS)


def write_ico(path: str, images: list) -> None:
    pngs = []
    for im in images:
        buf = io.BytesIO()
        im.save(buf, format="PNG")
        pngs.append(buf.getvalue())
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = b""
    for im, data in zip(images, pngs):
        w, h = im.size
        entries += struct.pack("<BBBBHHII", w % 256, h % 256, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    with open(path, "wb") as f:
        f.write(header + entries + b"".join(pngs))


def paused(im: Image.Image) -> Image.Image:
    r, g, b, a = im.split()
    gray = Image.merge("RGB", (r, g, b)).convert("L")
    gray = gray.point(lambda v: int(96 + v * 0.55))  # lift and flatten: reads as "inactive"
    a = a.point(lambda v: int(v * 0.8))
    return Image.merge("RGBA", (gray, gray, gray, a))


if __name__ == "__main__":
    imgs = [draw(n) for n in SIZES]
    write_ico("assets/best_loupe.ico", imgs)
    write_ico("assets/best_loupe_paused.ico", [paused(im) for im in imgs])
    if len(sys.argv) < 2:
        sys.exit(0)
    # Optional preview strip on light and dark backgrounds.
    strip = Image.new("RGBA", (sum(SIZES) + sum(SIZES[:5]) + 10 * (len(SIZES) + 5), 2 * 266), (0, 0, 0, 0))
    x = 0
    imgs = imgs[:5] + [paused(im) for im in imgs[:5]] + imgs[5:]
    for n, im in zip(SIZES[:5] * 2 + SIZES[5:], imgs):
        for row, bg in enumerate(((243, 243, 243, 255), (32, 32, 32, 255))):
            tile = Image.new("RGBA", (n + 10, 266), bg)
            tile.alpha_composite(im, (5, 5))
            strip.paste(tile, (x, row * 266))
        x += n + 10
    strip.save(sys.argv[1])
