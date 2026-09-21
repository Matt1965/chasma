#!/usr/bin/env python3
"""Insert missing item_catalog arguments in test call sites."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "src"
ITEM = "&crate::world::ItemCatalog::default(),"


def already_has_item_catalog_after(text: str, pos: int) -> bool:
    rest = text[pos : pos + 400]
    return bool(re.search(r"ItemCatalog|item_catalog", rest.split("\n")[0]))


def insert_after_match(text: str, match: re.Match, insert_line: str) -> str:
    end = match.end()
    if already_has_item_catalog_after(text, end):
        return text
    indent = match.group(1) if match.lastindex and match.lastindex >= 1 else "            "
    insertion = f"\n{indent}{insert_line}"
    return text[:end] + insertion + text[end:]


def fix_weapon_then_doodad(content: str) -> str:
    weapon_exprs = [
        r"&weapons\(\)",
        r"&weapons\b",
        r"&WeaponCatalog::default\(\)",
        r"weapon_catalog",
        r"&weapon_catalog",
    ]
    doodad_exprs = [
        r"&DoodadCatalog::default\(\)",
        r"&doodad_catalog\b",
    ]
    for weapon in weapon_exprs:
        for doodad in doodad_exprs:
            pattern = re.compile(
                rf"({weapon},)\n(\s+)(?!(?:&crate::world::)?ItemCatalog::|item_catalog,)({doodad},)"
            )
            content = pattern.sub(
                rf"\1\n\2{ITEM}\n\2\3",
                content,
            )
    return content


def fix_weapon_then_passability(content: str) -> str:
    next_exprs = [
        r"bundle\.catalogs\(\)",
        r"default_passability\(\)",
        r"TestPassabilityBundle::new\(\)\.catalogs\(\)",
        r"passability\(\)",
    ]
    for nxt in next_exprs:
        pattern = re.compile(
            rf"(&weapons\(\),)\n(\s+)(?!(?:&crate::world::)?ItemCatalog::|item_catalog,)({nxt})"
        )
        content = pattern.sub(rf"\1\n\2{ITEM}\n\2\3", content)
        pattern = re.compile(
            rf"(&weapons,)\n(\s+)(?!(?:&crate::world::)?ItemCatalog::|item_catalog,)({nxt})"
        )
        content = pattern.sub(rf"\1\n\2{ITEM}\n\2\3", content)
    return content


def fix_build_selected_panel_snapshot(content: str) -> str:
    pattern = re.compile(
        r"(build_selected_panel_snapshot\([\s\S]*?"
        r"(&WeaponCatalog::[^\n]+|&default_weapons\(\)[^\n]*),)\n"
        r"(\s+)\)"
    )

    def repl(m: re.Match) -> str:
        if "ItemCatalog" in m.group(0):
            return m.group(0)
        return f"{m.group(1)}\n{m.group(3)}{ITEM}\n{m.group(3)})"

    return pattern.sub(repl, content)


def fix_validate_attack_cycle(content: str) -> str:
    pattern = re.compile(
        r"(validate_attack_cycle_for_strike\(\s*"
        r"&[^,]+,\s*"
        r"[^,]+,\s*"
        r"&[^,]+,)\s*"
        r"(&[^,]+,\s*policy\(\))"
    )

    def repl(m: re.Match) -> str:
        if "ItemCatalog" in m.group(0):
            return m.group(0)
        return f"{m.group(1)}\n            {ITEM}\n            {m.group(2)}"

    return pattern.sub(repl, content)


def fix_weapon_for_unit_record_old_3arg(content: str) -> str:
    # weapon_for_unit_record(attacker, catalog, weapons) in tests
    pattern = re.compile(
        r"weapon_for_unit_record\(\s*"
        r"(?!world\b|&world\b)"
        r"([^,]+),\s*"
        r"(&?[^,]+),\s*"
        r"(&?weapons[^)]*)\)"
    )

    def repl(m: re.Match) -> str:
        attacker = m.group(1).strip()
        catalog = m.group(2).strip()
        weapons = m.group(3).strip()
        if "world" in attacker.lower() and "get_unit" not in attacker:
            return m.group(0)
        world_arg = "world"
        if attacker.startswith("&"):
            world_arg = "world"
        elif "get_unit" in attacker:
            world_arg = "world"
        return (
            f"weapon_for_unit_record({world_arg}, {attacker}, {catalog}, "
            f"{ITEM}, {weapons})"
        )

    return pattern.sub(repl, content)


def fix_weapon_for_unit_record_get_unit(content: str) -> str:
    pattern = re.compile(
        r"weapon_for_unit_record\(\s*"
        r"(world\.get_unit\([^)]+\)\.unwrap\(\)),\s*"
        r"(&catalog|&[^,]+),\s*"
        r"(&weapons[^)]*)\)"
    )
    return pattern.sub(
        rf"weapon_for_unit_record(world, \1, \2, {ITEM}, \3)",
        content,
    )


def fix_issue_selection_commands(content: str) -> str:
    for fn in (
        "issue_move_orders_to_selection",
        "issue_attack_move_orders_to_selection",
    ):
        pattern = re.compile(
            rf"({fn}\(\s*"
            rf"&mut world,\s*"
            rf"&selection,\s*"
            rf"&catalog,\s*"
            rf"&WeaponCatalog::[^\n]+,)\n"
            rf"(\s+)(&DoodadCatalog::[^\n]+,)"
        )
        content = pattern.sub(rf"\1\n\2{ITEM}\n\2\3", content)

    pattern = re.compile(
        r"(issue_attack_orders_to_selection\(\s*"
        r"&mut world,\s*"
        r"&selection,\s*"
        r"&catalog,\s*"
        r"&WeaponCatalog::[^\n]+,)\n"
        r"(\s+)(&DoodadCatalog::[^\n]+,)"
    )
    content = pattern.sub(rf"\1\n\2{ITEM}\n\2\3", content)
    return content


def fix_ctx_items_method(content: str) -> str:
    return content.replace("ctx.items()", "ctx.items")


def process_file(path: Path) -> bool:
    original = path.read_text(encoding="utf-8")
    content = original
    content = fix_weapon_then_doodad(content)
    content = fix_weapon_then_passability(content)
    content = fix_build_selected_panel_snapshot(content)
    content = fix_validate_attack_cycle(content)
    content = fix_weapon_for_unit_record_get_unit(content)
    content = fix_weapon_for_unit_record_old_3arg(content)
    content = fix_issue_selection_commands(content)
    content = fix_ctx_items_method(content)
    if content != original:
        path.write_text(content, encoding="utf-8")
        return True
    return False


def main() -> None:
    changed = []
    for path in ROOT.rglob("*.rs"):
        if process_file(path):
            changed.append(path.relative_to(ROOT.parent))
    print(f"Changed {len(changed)} files")
    for p in sorted(changed):
        print(p)


if __name__ == "__main__":
    main()
