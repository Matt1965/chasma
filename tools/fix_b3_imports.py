"""Add missing B3 imports to test modules."""

from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SRC = ROOT / "src"

SYMS = ("BuildingCatalog", "FootprintCatalog", "PassabilityCatalogs", "DoodadCatalog")


def fix_file(path: pathlib.Path) -> bool:
    text = path.read_text(encoding="utf-8")
    if "#[cfg(test)]" not in text:
        return False
    orig = text

    test_start = text.find("#[cfg(test)]")
    test_body = text[test_start:]
    needed = [s for s in SYMS if f"{s}::" in test_body or f"{s} {{" in test_body or f"{s}<" in test_body]
    if not needed:
        return False

    def patch_use(m: re.Match[str]) -> str:
        block = m.group(0)
        if not block.startswith("use crate::world::"):
            return block
        names = set(re.findall(r"\b([A-Z][A-Za-z0-9_]*)\b", block))
        missing = [s for s in needed if s not in names]
        if not missing:
            return block
        lines = block.splitlines()
        if len(lines) == 1:
            return block
        inner = lines[1:-1]
        if len(inner) == 1 and "{" in inner[0]:
            content = inner[0].strip().strip("{").strip().strip("}").strip()
            parts = [p.strip() for p in content.split(",") if p.strip()]
            parts.extend(missing)
            parts = sorted(set(parts), key=str.lower)
            return f"{lines[0]}\n        {', '.join(parts)},\n    {lines[-1]}"
        return block

    text = re.sub(r"use crate::world::\{[^}]+\};", patch_use, text[test_start:], count=3)
    if text == test_body:
        return False
    text = text[:test_start] + text
    path.write_text(text, encoding="utf-8")
    return True


def main() -> None:
    for path in SRC.rglob("*.rs"):
        if fix_file(path):
            print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
