"""Fix broken move_doodad patches that put None inside Vec3::new."""

from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SRC = ROOT / "src"

PATTERN = re.compile(
    r"(move_doodad\(&mut world,\s*[^,]+,\s*(?:pos|position)\([^)]*Vec3::new\([^,]+,\s*[^,]+,\s*[^,]+),\s*None\)\),)"
)


def fix_file(path: pathlib.Path) -> bool:
    text = path.read_text(encoding="utf-8")
    orig = text

    text = re.sub(
        r"Vec3::new\(([^,]+),\s*([^,]+),\s*([^,]+),\s*None\)\)",
        r"Vec3::new(\1, \2, \3))",
        text,
    )

    # add None occupancy arg when move_doodad ends with )) without None
    text = re.sub(
        r"(move_doodad\(\s*&mut world,\s*[^,]+,\s*(?:pos|position)\([^)]+\)\))\s*\)",
        r"\1, None)",
        text,
    )

    if text != orig:
        path.write_text(text, encoding="utf-8")
        return True
    return False


def main() -> None:
    for path in SRC.rglob("*.rs"):
        if "move_doodad" in path.read_text(encoding="utf-8"):
            if fix_file(path):
                print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
