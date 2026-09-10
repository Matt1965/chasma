"""Offline composite of the bottom HUD from the exported chrome.

Reproduces the Bevy draw order and nine-slice behaviour against the geometry
that `dump_hud_geometry` reports, so the frame silhouette can be reviewed
without a running client. Developer aid only; nothing ships from here.
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
HUD = ROOT / "assets" / "images" / "ui" / "hud"

# From `cargo test dump_hud_geometry` at 1920x1080.
ROOT_RECT = (0, 884, 1920, 1080)
BAR = (53, 896, 1893, 1066)
SECTIONS = {
    "selected": (53, 896, 425, 1066),
    "roster": (425, 896, 1325, 1066),
    "commands": (1325, 896, 1725, 1066),
    "utility": (1725, 896, 1893, 1066),
}
# Clip rect plus which sides are interior joins (image overhangs the clip there,
# trimming the deep chamfer to the shallow join chamfer). Draw order: shorter
# plates last.
PLATES = [
    ("Selected", (0, 884, 434, 1080), (False, True)),
    ("Utility", (1716, 886, 1920, 1080), (True, False)),
    ("Roster", (416, 889, 1334, 1073), (True, True)),
    ("Commands", (1316, 894, 1734, 1070), (True, True)),
]
JOIN_OVERLAP = 9

BACKING_INSET_X = 14
BACKING_INSET_Y = 12
PLATE_CORNER = 18
ENDCAP_LEFT_W = 53
ENDCAP_RIGHT_W = 27
SPIRE_W = 21
SPIRE_H = 95

VIEW_TOP = 770


def nine_slice(image: Image.Image, width: int, height: int, corner: int) -> Image.Image:
    width = max(width, corner * 2 + 1)
    height = max(height, corner * 2 + 1)
    sw, sh = image.size
    out = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    inner_w = width - 2 * corner
    inner_h = height - 2 * corner
    out.paste(image.crop((corner, corner, sw - corner, sh - corner)).resize((inner_w, inner_h)), (corner, corner))
    out.paste(image.crop((corner, 0, sw - corner, corner)).resize((inner_w, corner)), (corner, 0))
    out.paste(image.crop((corner, sh - corner, sw - corner, sh)).resize((inner_w, corner)), (corner, height - corner))
    out.paste(image.crop((0, corner, corner, sh - corner)).resize((corner, inner_h)), (0, corner))
    out.paste(image.crop((sw - corner, corner, sw, sh - corner)).resize((corner, inner_h)), (width - corner, corner))
    out.paste(image.crop((0, 0, corner, corner)), (0, 0))
    out.paste(image.crop((sw - corner, 0, sw, corner)), (width - corner, 0))
    out.paste(image.crop((0, sh - corner, corner, sh)), (0, height - corner))
    out.paste(image.crop((sw - corner, sh - corner, sw, sh)), (width - corner, height - corner))
    return out


def over(canvas: Image.Image, layer: Image.Image, x: int, y: int) -> None:
    patch = canvas.crop((x, y, x + layer.width, y + layer.height))
    canvas.paste(Image.alpha_composite(patch, layer), (x, y))


def main() -> None:
    width = ROOT_RECT[2]
    height = ROOT_RECT[3] - VIEW_TOP
    canvas = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    # Backdrop stands in for the world so transparent gaps show up loudly.
    draw = ImageDraw.Draw(canvas)
    for row in range(height):
        t = row / height
        draw.line(
            [(0, row), (width, row)],
            fill=(int(70 + 40 * t), int(130 + 30 * t), int(150 - 20 * t), 255),
        )

    def to_view(rect):
        return rect[0], rect[1] - VIEW_TOP, rect[2], rect[3] - VIEW_TOP

    plate_src = Image.open(HUD / "plate_frame.png").convert("RGBA")
    backing_src = Image.open(HUD / "plate_backing.png").convert("RGBA")

    # ZIndex(-3): continuous interior backing.
    rx0, ry0, rx1, ry1 = to_view(ROOT_RECT)
    bx0, by0 = rx0 + BACKING_INSET_X, ry0 + BACKING_INSET_Y
    bx1, by1 = rx1 - BACKING_INSET_X, ry1 - BACKING_INSET_Y
    over(canvas, backing_src.resize((bx1 - bx0, by1 - by0)), bx0, by0)

    # One octagonal plate per section, shorter plates last. Squared sides are
    # produced by overhanging the frame image and clipping it, exactly as the
    # Bevy clip container does.
    for _name, rect, (interior_left, interior_right) in PLATES:
        x0, y0, x1, y1 = to_view(rect)
        clip_w, clip_h = x1 - x0, y1 - y0
        over_l = JOIN_OVERLAP if interior_left else 0
        over_r = JOIN_OVERLAP if interior_right else 0
        art = nine_slice(plate_src, clip_w + over_l + over_r, clip_h, PLATE_CORNER)
        over(canvas, art.crop((over_l, 0, over_l + clip_w, clip_h)), x0, y0)

    # ZIndex(-1): ornaments.
    endcap_l = Image.open(HUD / "endcap_left.png").convert("RGBA")
    endcap_r = Image.open(HUD / "endcap_right.png").convert("RGBA")
    spire = Image.open(HUD / "endcap_spire.png").convert("RGBA")
    band_h = ry1 - ry0
    over(canvas, endcap_l.resize((ENDCAP_LEFT_W, band_h)), 0, ry0)
    over(canvas, endcap_r.resize((ENDCAP_RIGHT_W, band_h)), width - ENDCAP_RIGHT_W, ry0)
    over(canvas, spire.resize((SPIRE_W, SPIRE_H)), width - ENDCAP_RIGHT_W - SPIRE_W, ry0 - SPIRE_H)

    # Content stand-ins so the frame is judged with the row populated.
    draw = ImageDraw.Draw(canvas)
    for name, rect in SECTIONS.items():
        x0, y0, x1, y1 = to_view(rect)
        draw.rectangle([x0 + 10, y0 + 8, x1 - 10, y1 - 8], outline=(120, 105, 85, 90))
        draw.text((x0 + 16, y0 + 12), name, fill=(150, 150, 150, 255))

    out = ROOT / "assets" / "images" / "ui" / "hud" / "_preview_hud.png"
    canvas.convert("RGB").save(out)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
