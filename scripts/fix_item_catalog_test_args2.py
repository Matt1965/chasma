#!/usr/bin/env python3
"""Second pass: fix over-insertions and remaining missing item_catalog args."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "src"
ITEM = "&crate::world::ItemCatalog::default()"
ITEM_LINE = f"            {ITEM},"


def remove_run_sim_tick_duplicate_item_catalog(content: str) -> str:
    pattern = re.compile(
        r"(run_simulation_tick\(\s*"
        r"(?:&mut world|world|&mut self\.world)[\s\S]*?"
        r"(?:&weapons\(\)|&weapons\b|weapon_catalog|&weapon_catalog)\s*,)\s*\n"
        r"\s*&crate::world::ItemCatalog::default\(\),\s*\n"
        r"(\s*&(?:crate::world::)?DoodadCatalog)",
        re.MULTILINE,
    )
    return pattern.sub(r"\1\n\2", content)


def fix_all(content: str) -> str:
    content = content.replace(",,", ",")
    content = remove_run_sim_tick_duplicate_item_catalog(content)

    for fn in (
        "try_dispatch_owned_building_player_interaction",
        "assign_hauling_task",
        "assign_hauling_task_with_priority",
    ):
        content = re.sub(
            rf"({fn}\([\s\S]*?&WeaponCatalog::[^\n]+,)\n"
            rf"\s*&crate::world::ItemCatalog::default\(\),\n"
            rf"(\s+&DoodadCatalog)",
            r"\1\n\2",
            content,
        )
        content = re.sub(
            rf"({fn}\([\s\S]*?weapon_catalog,\n)"
            rf"\s*&crate::world::ItemCatalog::default\(\),\n"
            rf"(\s+&doodad_catalog)",
            r"\1\2",
            content,
            flags=re.IGNORECASE,
        )

    for fn in ("run_frames", "issue_move", "issue_player_move"):
        content = re.sub(
            rf"({fn}\([\s\S]*?&weapon_catalog,\n)"
            rf"\s*&crate::world::ItemCatalog::default\(\),\n"
            rf"(\s+&doodad_catalog)",
            r"\1\2",
            content,
            flags=re.IGNORECASE,
        )

    # issue_unit_order missing item_catalog
    content = re.sub(
        r"(issue_unit_order\(\s*\n"
        r"[\s\S]*?"
        r"(?:&weapons\(\)|&weapons\b|weapons,|&WeaponCatalog::[^\n]+|weapon_catalog,)\s*\n)"
        r"(\s+)(?!&crate::world::ItemCatalog|item_catalog)(&(?:crate::world::)?DoodadCatalog)",
        rf"\1\2{ITEM},\n\2\3",
        content,
    )

    def insert_before_policy(fn: str, content: str) -> str:
        return re.sub(
            rf"({fn}\([^)]*"
            rf"&[^,\n]+,\s*"
            rf"&[^,\n]+,\s*"
            rf")(\s*(?:AttackTargetingPolicy|policy\(\)))",
            rf"\1{ITEM},\2",
            content,
        )

    for fn in (
        "find_auto_acquire_target",
        "validate_mechanical_attack_target",
        "validate_explicit_attack_target",
        "scan_attack_move_target",
    ):
        content = insert_before_policy(fn, content)

    content = re.sub(
        r"(classify_unit_target\([^)]*"
        r"&[^,\n]+,\s*"
        r"&[^,\n]+,\s*"
        r")(\s*(?:AttackTargetingPolicy|policy\(\)))",
        rf"\1{ITEM},\2",
        content,
    )

    content = re.sub(
        r"range_check_for_units\(&([^,]+), ([^,]+), ([^,]+), (&[^,]+), (&[^)]+)\)",
        rf"range_check_for_units(&\1, \2, \3, \4, {ITEM}, \5)",
        content,
    )

    content = content.replace(
        """        issue_unit_order(
            world,
            catalog,
            weapon_catalog,
            doodad_catalog,""",
        f"""        issue_unit_order(
            world,
            catalog,
            weapon_catalog,
            {ITEM},
            doodad_catalog,""",
    )

    content = content.replace(
        """        build_selected_panel_snapshot(
            world_selection,
            selected_units,
            world,
            &wolf_catalog(),
            &building_catalog(),
            &default_weapons(),
        )""",
        f"""        build_selected_panel_snapshot(
            world_selection,
            selected_units,
            world,
            &wolf_catalog(),
            &building_catalog(),
            &default_weapons(),
            {ITEM},
        )""",
    )

    return content


def process_file(path: Path) -> bool:
    original = path.read_text(encoding="utf-8")
    content = fix_all(original)
    if content != original:
        path.write_text(content, encoding="utf-8")
        return True
    return False


def main() -> None:
    changed = sum(process_file(p) for p in ROOT.rglob("*.rs"))
    print(f"Changed {changed} files")


if __name__ == "__main__":
    main()
