#!/usr/bin/env python3
"""Insert empty AppearanceProfileCatalog into create_unit* and import_units calls."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "src"

CREATE_FUNCS = [
    "create_unit",
    "create_unit_with_ownership",
    "create_unit_with_inventory",
]

EMPTY = "&crate::world::AppearanceProfileCatalog::empty(),"


def already_has_appearance(chunk: str) -> bool:
    return "AppearanceProfileCatalog" in chunk


def insert_after_first_catalog_arg(text: str, fn: str) -> str:
    pattern = re.compile(
        rf"({fn}\(\s*\n\s*&(?:self\.)?(?:\w+),\s*\n\s*)(&mut (?:self\.)?(?:world)|(?:self\.)?world,)",
        re.MULTILINE,
    )

    def repl(match: re.Match[str]) -> str:
        if already_has_appearance(match.group(0)):
            return match.group(0)
        return f"{match.group(1)}{EMPTY}\n        {match.group(2)}"

    text = pattern.sub(repl, text)

    pattern2 = re.compile(
        rf"({fn}\(\s*)(\w+),\s*(&mut world|world,)",
        re.MULTILINE,
    )

    def repl2(match: re.Match[str]) -> str:
        if already_has_appearance(match.group(0)):
            return match.group(0)
        return f"{match.group(1)}{match.group(2)}, {EMPTY} {match.group(3)}"

    return pattern2.sub(repl2, text)


def insert_import_appearance(text: str) -> str:
    pattern = re.compile(
        r"(&crate::world::InventoryProfileCatalog::default\(\),)\s*\n(\s*)\)",
        re.MULTILINE,
    )

    def repl(match: re.Match[str]) -> str:
        if "AppearanceProfileCatalog" in match.group(0):
            return match.group(0)
        return f"{match.group(1)}\n{match.group(2)}{EMPTY}\n{match.group(2)})"

    text = pattern.sub(repl, text)

    pattern2 = re.compile(
        r"(&profiles,)\s*\n(\s*)\)\s*\n\s*\.unwrap\(\);",
        re.MULTILINE,
    )

    def repl2(match: re.Match[str]) -> str:
        if "AppearanceProfileCatalog" in match.group(0):
            return match.group(0)
        return f"{match.group(1)}\n{match.group(2)}{EMPTY}\n{match.group(2)})\n        .unwrap();"

    return pattern2.sub(repl2, text)


for path in ROOT.rglob("*.rs"):
    text = path.read_text(encoding="utf-8")
    orig = text
    for fn in CREATE_FUNCS:
        text = insert_after_first_catalog_arg(text, fn)
    text = insert_import_appearance(text)
    if text != orig:
        path.write_text(text, encoding="utf-8")
        print(f"updated {path.relative_to(ROOT.parent)}")
