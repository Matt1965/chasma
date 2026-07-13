"""Restore building/footprint args on run_simulation_tick call sites."""

from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SRC = ROOT / "src"

# Wrong: PassabilityCatalogs passed where doodad_catalog belongs
WRONG_PASS = re.compile(
    r"run_simulation_tick\(\s*"
    r"([^,]+),\s*"
    r"([^,]+),\s*"
    r"([^,]+),\s*"
    r"PassabilityCatalogs\s*\{[^}]+\},\s*"
    r"(&NavigationConfig[^,]+),",
    re.MULTILINE | re.DOTALL,
)

INSERT_AFTER_DOODAD = re.compile(
    r"(run_simulation_tick\(\s*"
    r"(?:&mut )?[^,]+,\s*"
    r"[^,]+,\s*"
    r"[^,]+,\s*"
    r"(?:&doodad_catalog|&DoodadCatalog::default\(\)|&doodads|PassabilityCatalogs\s*\{[^}]+\}),\s*)"
    r"(&NavigationConfig)",
    re.MULTILINE | re.DOTALL,
)


def fix_file(path: pathlib.Path) -> bool:
    text = path.read_text(encoding="utf-8")
    orig = text

    def wrong_repl(m: re.Match[str]) -> str:
        return (
            f"run_simulation_tick(\n                {m.group(1).strip()},\n"
            f"                {m.group(2).strip()},\n"
            f"                {m.group(3).strip()},\n"
            f"                &DoodadCatalog::default(),\n"
            f"                &BuildingCatalog::default(),\n"
            f"                &FootprintCatalog::default(),\n"
            f"                {m.group(4).strip()},"
        )

    text = WRONG_PASS.sub(wrong_repl, text)

    def insert_repl(m: re.Match[str]) -> str:
        head = m.group(1)
        if "BuildingCatalog::default()" in head:
            return m.group(0)
        if "PassabilityCatalogs" in head:
            return m.group(0)  # handled above
        return (
            f"{head}"
            f"&BuildingCatalog::default(),\n                "
            f"&FootprintCatalog::default(),\n                "
            f"{m.group(2)}"
        )

    text = INSERT_AFTER_DOODAD.sub(insert_repl, text)

    if text != orig:
        path.write_text(text, encoding="utf-8")
        return True
    return False


def main() -> None:
    for path in SRC.rglob("*.rs"):
        if "run_simulation_tick" in path.read_text(encoding="utf-8"):
            if fix_file(path):
                print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
