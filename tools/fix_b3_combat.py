"""Fix incorrect B3 script patches for combat APIs."""

from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SRC = ROOT / "src"

PASS = """PassabilityCatalogs {
            doodad: &DoodadCatalog::default(),
            building: &BuildingCatalog::default(),
            footprint: &FootprintCatalog::default(),
        }"""

STRIKE_EXTRA = re.compile(
    r"(&DoodadCatalog::default\(\),)\n\s*&BuildingCatalog::default\(\),\n\s*&FootprintCatalog::default\(\),\n(\s*&NavigationConfig)",
    re.MULTILINE,
)

ENGAGEMENT_OLD = re.compile(
    r"(&DoodadCatalog::default\(\),)\n\s*&BuildingCatalog::default\(\),\n\s*&FootprintCatalog::default\(\),\n(\s*&NavigationConfig)",
    re.MULTILINE,
)


def fix_file(path: pathlib.Path) -> bool:
    text = path.read_text(encoding="utf-8")
    orig = text

    if "step_all_combat_strikes" in text:
        text = STRIKE_EXTRA.sub(r"\1\n\2", text)

    if "step_all_combat_engagement" in text:
        text = ENGAGEMENT_OLD.sub(f"{PASS},\n\\2", text)

    # death.rs style without BuildingCatalog lines
    text = re.sub(
        r"step_all_combat_engagement\(\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*(&crate::world::DoodadCatalog::default\(\)|&DoodadCatalog::default\(\)),\s*(&crate::world::NavigationConfig::default\(\)|&NavigationConfig::default\(\)),",
        lambda m: (
            f"step_all_combat_engagement(\n            {m.group(1).strip()},\n"
            f"            {m.group(2).strip()},\n            {m.group(3).strip()},\n"
            f"            {PASS},\n            {m.group(5).strip()},"
        ),
        text,
    )

    if text != orig:
        path.write_text(text, encoding="utf-8")
        return True
    return False


def main() -> None:
    for path in SRC.rglob("*.rs"):
        if fix_file(path):
            print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
