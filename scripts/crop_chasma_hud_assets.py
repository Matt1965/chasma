"""Derive bottom HUD chrome from assets/images/chasma_ui.png (composite mockup).

Band geometry
-------------
The mockup HUD band is y 567..766 (199px) on a 1983px-wide composite.

The mockup is four interlocking plates, not one rectangle:

    plate      x range        top inset  bot inset  height
    selected    35..460           0           2        197
    roster     500..1220          3          12        184
    command   1280..1780         25          14        160
    utility   1880..1955          0           2        197

The far left (x=0..31) is a tapered ornamental nose, not a vertical cut.
Earlier passes drew a rectangular plate and backing through that taper,
which is why the live left edge looked clipped.

Frame composition
-----------------
`plate_frame` is a nine-sliceable octagon: real mockup rail pixels, real
stone interior, transparent *outside* the chamfer. Each section draws
this as an overlay, so the visible silhouette is the octagon — not a
rectangle with decorations on it. `plate_backing` is the same interior
material, used as the per-plate fill when the outline and fill are
split; the live HUD uses the combined plate image.

Buttons, roster cards, and inner content plates are not exported.
"""

from __future__ import annotations

import math
import statistics
import struct
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "assets" / "images" / "chasma_ui.png"
OUT = ROOT / "assets" / "images" / "ui" / "hud"

BAND_TOP = 567
BAND_BOTTOM = 766
BAND_HEIGHT = BAND_BOTTOM - BAND_TOP

INTERIOR_SAMPLE_X = (352, 364)
INTERIOR_SAMPLE_Y = (640, 664)
BACKING_SAMPLE_Y = (579, 752)

# Clean rail probe columns, free of portraits / buttons / empty-slot outlines.
RAIL_SAMPLE_X = (300, 324)
# Vertical 9-slice sides stretch to plate height. Sample the flat interior,
# not the bright left bronze, or every join becomes a full-height brace.
SIDE_RAIL_X = 310

ENDCAP_LEFT = (0, BAND_TOP, 54, BAND_BOTTOM)
ENDCAP_RIGHT = (1959, BAND_TOP, 1983, BAND_BOTTOM)
ENDCAP_SPIRE = (1927, 472, 1948, BAND_TOP + 1)

# Nine-slice: corner must contain the full chamfer so diagonals never stretch.
PLATE_CORNER_PX = 32
PLATE_MIDDLE_PX = 10
PLATE_CHAMFER_PX = 26
PLATE_SIZE_PX = PLATE_CORNER_PX * 2 + PLATE_MIDDLE_PX
# How many pixels inward from the octagon edge stay as rail (then transparent).
FRAME_RING_PX = 12

STALE = [
    "panel_fill",
    "separator_v",
    "bottom_strip",
    "roster_card",
    "cmd_button",
    "utility_button",
    "frame_mid",
    "divider_v",
]

FRAME_MIN_LUMINANCE_RANGE = 60.0
FRAME_MIN_LUMINANCE_VARIANCE = 150.0
BACKING_MIN_LUMINANCE_VARIANCE = 4.0


def luminance_stats(image: Image.Image) -> tuple[float, float, float]:
    px = image.load()
    width, height = image.size
    samples: list[float] = []
    for y in range(height):
        for x in range(width):
            r, g, b, a = px[x, y]
            if a < 32:
                continue
            samples.append((r + g + b) / 3.0)
    if not samples:
        return 0.0, 0.0, 0.0
    return statistics.pvariance(samples), min(samples), max(samples)


def _avg_row(image: Image.Image, y: int, x0: int, x1: int) -> tuple[int, int, int, int]:
    px = image.load()
    totals = [0, 0, 0, 0]
    count = 0
    for x in range(x0, x1):
        pixel = px[x, y]
        for i in range(4):
            totals[i] += pixel[i]
        count += 1
    return tuple(value // count for value in totals)  # type: ignore[return-value]


def sample_rails(image: Image.Image) -> dict[str, list[tuple[int, int, int, int]]]:
    """Real mockup rail pixels, indexed by distance from the outer edge."""
    x0, x1 = RAIL_SAMPLE_X
    top = [_avg_row(image, BAND_TOP + depth, x0, x1) for depth in range(FRAME_RING_PX)]
    bottom = [
        _avg_row(image, BAND_BOTTOM - 1 - depth, x0, x1) for depth in range(FRAME_RING_PX)
    ]
    px = image.load()
    side = []
    for depth in range(FRAME_RING_PX):
        totals = [0, 0, 0, 0]
        count = 0
        x = SIDE_RAIL_X + min(depth, 4)
        for y in range(BAND_TOP + 20, BAND_BOTTOM - 20):
            pixel = px[x, y]
            if pixel[3] < 32:
                continue
            for i in range(4):
                totals[i] += pixel[i]
            count += 1
        side.append(tuple(value // max(count, 1) for value in totals))  # type: ignore[return-value]
    return {"top": top, "bottom": bottom, "side": side}


def compose_plate_backing(image: Image.Image) -> Image.Image:
    ix0, ix1 = INTERIOR_SAMPLE_X
    by0, by1 = BACKING_SAMPLE_Y
    return image.crop((ix0, by0, ix1, by1))


def _edge_field(x: int, y: int, size: int) -> tuple[float, str]:
    last = size - 1
    chamfer = PLATE_CHAMFER_PX
    diag = math.sqrt(2.0)
    candidates = [
        (float(y), "top"),
        (float(last - y), "bottom"),
        (float(x), "side"),
        (float(last - x), "side"),
        ((x + y - chamfer) / diag, "top"),
        (((last - x) + y - chamfer) / diag, "top"),
        ((x + (last - y) - chamfer) / diag, "bottom"),
        (((last - x) + (last - y) - chamfer) / diag, "bottom"),
    ]
    return min(candidates, key=lambda item: item[0])


def compose_plate_frame(image: Image.Image) -> Image.Image:
    """Octagonal nine-slice: mockup rails + stone interior, cut corners."""
    size = PLATE_SIZE_PX
    rails = sample_rails(image)
    interior = image.crop(
        (
            INTERIOR_SAMPLE_X[0],
            INTERIOR_SAMPLE_Y[0],
            INTERIOR_SAMPLE_X[1],
            INTERIOR_SAMPLE_Y[1],
        )
    )
    interior_px = interior.load()
    iw, ih = interior.size
    frame = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    out = frame.load()

    for y in range(size):
        for x in range(size):
            depth, edge = _edge_field(x, y, size)
            if depth < -0.5:
                continue
            band = int(math.floor(depth + 0.5))
            if band < FRAME_RING_PX:
                r, g, b, a = rails[edge][band]
                if a < 16:
                    continue
                out[x, y] = (r, g, b, max(a, 220))
                continue
            r, g, b, _ = interior_px[x % iw, y % ih]
            out[x, y] = (r, g, b, 253)

    return frame


def validate_plate_frame(image: Image.Image) -> None:
    width, height = image.size
    if width != PLATE_SIZE_PX or height != PLATE_SIZE_PX:
        raise ValueError(f"plate frame must be {PLATE_SIZE_PX}px square, got {width}x{height}")

    px = image.load()
    if px[0, 0][3] > 16 or px[width - 1, height - 1][3] > 16:
        raise ValueError("plate frame corners are not chamfered")
    if px[width // 2, height // 2][3] < 200:
        raise ValueError("plate frame interior must be opaque")
    mid_top = px[width // 2, 2]
    if mid_top[3] < 80:
        raise ValueError("plate frame top rail is missing")

    variance, lum_min, lum_max = luminance_stats(image)
    lum_range = lum_max - lum_min
    if lum_range < FRAME_MIN_LUMINANCE_RANGE:
        raise ValueError(
            f"plate frame lacks trim contrast: range {lum_range:.1f} < {FRAME_MIN_LUMINANCE_RANGE}"
        )
    if variance < FRAME_MIN_LUMINANCE_VARIANCE:
        raise ValueError(
            f"plate frame looks flat: variance {variance:.1f} < {FRAME_MIN_LUMINANCE_VARIANCE}"
        )


def validate_backing(image: Image.Image) -> None:
    width, height = image.size
    if width < 4 or height < 64:
        raise ValueError(f"backing strip too small: {width}x{height}")
    variance, _, _ = luminance_stats(image)
    if variance < BACKING_MIN_LUMINANCE_VARIANCE:
        raise ValueError(
            f"backing strip is flat; expected real interior gradient (variance {variance:.1f})"
        )


def png_dimensions(path: Path) -> tuple[int, int]:
    with path.open("rb") as handle:
        signature = handle.read(8)
        if signature != b"\x89PNG\r\n\x1a\n":
            raise ValueError(f"{path} is not a PNG")
        _length, chunk_type = struct.unpack(">I4s", handle.read(8))
        if chunk_type != b"IHDR":
            raise ValueError(f"{path} missing IHDR chunk")
        width, height = struct.unpack(">II", handle.read(8))
        return width, height


def write(name: str, image: Image.Image) -> None:
    path = OUT / f"{name}.png"
    image.save(path)
    print(f"wrote {path.name} size={image.size}")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    image = Image.open(SOURCE).convert("RGBA")

    print(f"band       = y {BAND_TOP}..{BAND_BOTTOM} (height {BAND_HEIGHT})")
    print(
        f"plate      = {PLATE_SIZE_PX}px square, corner {PLATE_CORNER_PX}px, "
        f"chamfer {PLATE_CHAMFER_PX}px, opaque interior, transparent outside"
    )

    plate_frame = compose_plate_frame(image)
    validate_plate_frame(plate_frame)

    plate_backing = compose_plate_backing(image)
    validate_backing(plate_backing)

    write("plate_frame", plate_frame)
    write("plate_backing", plate_backing)
    write("endcap_left", image.crop(ENDCAP_LEFT))
    write("endcap_right", image.crop(ENDCAP_RIGHT))
    write("endcap_spire", image.crop(ENDCAP_SPIRE))

    frame_var, frame_min, frame_max = luminance_stats(plate_frame)
    backing_var, _, _ = luminance_stats(plate_backing)
    print(f"plate ok   = lum range {frame_max - frame_min:.1f}, variance {frame_var:.1f}")
    print(f"backing ok = variance {backing_var:.1f} (real interior gradient)")

    for name in STALE:
        stale = OUT / f"{name}.png"
        if stale.exists():
            stale.unlink()
            print(f"removed superseded crop {stale.name}")

    for leftover in OUT.glob("_*.png"):
        leftover.unlink()
        print(f"removed debug crop {leftover.name}")

    width, height = png_dimensions(OUT / "plate_frame.png")
    assert (width, height) == plate_frame.size


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001 - script entrypoint
        print(f"error: {exc}", file=sys.stderr)
        raise
