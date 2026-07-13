"""Bulk-fix B3 API migration in test and inline helper call sites."""

from __future__ import annotations

import pathlib
import re

PASS = """PassabilityCatalogs {
            doodad: &{doodad},
            building: &BuildingCatalog::default(),
            footprint: &FootprintCatalog::default(),
        }"""

ROOT = pathlib.Path(__file__).resolve().parents[1]
SRC = ROOT / "src"


def add_world_imports(text: str) -> str:
    needs_building = "BuildingCatalog" in text and "use crate::world::" in text
    needs_footprint = "FootprintCatalog" in text and "use crate::world::" in text
    needs_passability = "PassabilityCatalogs" in text and "use crate::world::" in text
    if not (needs_building or needs_footprint or needs_passability):
        return text

    def patch_use_block(match: re.Match[str]) -> str:
        block = match.group(0)
        if "BuildingCatalog" in block and needs_building:
            return block
        items = [line.strip().rstrip(",") for line in block.splitlines()[1:-1]]
        names = set()
        for item in items:
            if " as " in item:
                names.add(item.split(" as ")[-1].strip())
            else:
                names.add(item.strip())
        additions = []
        if needs_building and "BuildingCatalog" not in names:
            additions.append("BuildingCatalog")
        if needs_footprint and "FootprintCatalog" not in names:
            additions.append("FootprintCatalog")
        if needs_passability and "PassabilityCatalogs" not in names:
            additions.append("PassabilityCatalogs")
        if not additions:
            return block
        first = block.splitlines()[0]
        last = block.splitlines()[-1]
        body = block.splitlines()[1:-1]
        # insert alphabetically into first line if single-line use
        if len(body) == 1 and "{" in body[0]:
            inner = body[0].strip().strip("{").strip().strip("}").strip()
            parts = [p.strip() for p in inner.split(",") if p.strip()]
            parts.extend(additions)
            parts = sorted(set(parts), key=str.lower)
            return f"{first}\n        {', '.join(parts)},\n    {last}"
        return block

    return re.sub(
        r"use crate::world::\{[^}]+\};",
        patch_use_block,
        text,
        count=1,
    )


def fix_file(path: pathlib.Path) -> bool:
    text = path.read_text(encoding="utf-8")
    orig = text

    # run_simulation_tick: insert building + footprint after doodad catalog arg
    text = re.sub(
        r"(&DoodadCatalog::default\(\)|&doodad_catalog),\n(\s*)(&NavigationConfig)",
        r"\1,\n\2&BuildingCatalog::default(),\n\2&FootprintCatalog::default(),\n\2\3",
        text,
    )

    # dispatch_one: same pattern (doodad then nav)
    text = re.sub(
        r"(&DoodadCatalog::default\(\)|&doodad_catalog),\n(\s*)(&NavigationConfig::default\(\)),\n(\s*)layout\(\)",
        r"\1,\n\2&BuildingCatalog::default(),\n\2&FootprintCatalog::default(),\n\2\3,\n\4layout()",
        text,
    )

    # step_all_combat_engagement: doodad -> passability
    text = re.sub(
        r"step_all_combat_engagement\(\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*&(\w+),\s*(&NavigationConfig[^,]+),",
        lambda m: (
            f"step_all_combat_engagement(\n            {m.group(1).strip()},\n"
            f"            {m.group(2).strip()},\n            {m.group(3).strip()},\n"
            f"            {PASS.format(doodad=m.group(4))},\n            {m.group(5).strip()},"
        ),
        text,
    )

    # step_all_unit_movement / step_unit_movement
    text = re.sub(
        r"step_all_unit_movement\(\s*(&mut world,\s*&\w+,\s*)&(\w+),",
        lambda m: f"step_all_unit_movement(\n            {m.group(1)}{PASS.format(doodad=m.group(2))},",
        text,
    )
    text = re.sub(
        r"step_unit_movement\(\s*(&mut world,\s*&\w+,\s*)&(\w+),",
        lambda m: f"step_unit_movement(\n            {m.group(1)}{PASS.format(doodad=m.group(2))},",
        text,
    )

    # resolve pending orders
    for fn in ("resolve_all_pending_unit_orders", "resolve_pending_unit_orders"):
        text = re.sub(
            rf"{fn}\(\s*(&mut world,\s*&\w+,\s*)&(\w+),\s*(&nav)",
            lambda m, fn=fn: (
                f"{fn}(\n        {m.group(1).strip()},\n"
                f"        {PASS.format(doodad=m.group(2))},\n        {m.group(3)}"
            ),
            text,
        )

    # find_path
    text = re.sub(
        r"find_path\(\s*&world,\s*&(\w+),\s*(&nav)",
        lambda m: f"find_path(\n        &world,\n        {PASS.format(doodad=m.group(1))},\n        {m.group(2)}",
        text,
    )

    # move_doodad / remove_doodad optional occupancy
    text = re.sub(
        r"move_doodad\(\s*(&mut world,\s*[^,]+,\s*[^)]+)\)",
        r"move_doodad(\1, None)",
        text,
    )
    text = re.sub(
        r"remove_doodad\(\s*(&mut world,\s*[^)]+)\)",
        r"remove_doodad(\1, None)",
        text,
    )

    # interaction contexts
    text = re.sub(
        r"InteractionQueryContext::new\(\s*([^,]+),\s*&(\w+),\s*&(\w+),\s*&(\w+)\)",
        r"InteractionQueryContext::new(\1, &\2, &BuildingCatalog::default(), &FootprintCatalog::default(), &\3, &\4)",
        text,
    )
    text = re.sub(
        r"InteractionResolveContext::new\(\s*([^,]+),\s*&(\w+),\s*&(\w+),\s*&(\w+),\s*([^)]+)\)",
        r"InteractionResolveContext::new(\1, &\2, &BuildingCatalog::default(), &FootprintCatalog::default(), &\3, &\4, \5)",
        text,
    )

    # obstacle query_tests duplicate import
    if path.name == "query_tests.rs":
        text = text.replace(
            "DoodadSource, FootprintCatalog, Heightfield",
            "DoodadSource, Heightfield",
        )

    # footprint.rs test imports
    if path.name == "footprint.rs" and "definition_id::BuildingDefinitionId" in text:
        text = text.replace(
            "use crate::world::building::catalog::definition_id::BuildingDefinitionId;\n"
            "    use crate::world::building::catalog::render_key::BuildingRenderKey;\n"
            "    use crate::world::building::category::BuildingCategoryId;",
            "use crate::world::building::catalog::BuildingDefinitionId;\n"
            "    use crate::world::building::catalog::BuildingRenderKey;\n"
            "    use crate::world::building::category::BuildingCategoryId;",
        )

    text = add_world_imports(text)

    if text != orig:
        path.write_text(text, encoding="utf-8")
        return True
    return False


def main() -> None:
    changed = []
    for path in SRC.rglob("*.rs"):
        if fix_file(path):
            changed.append(path)
    for path in changed:
        print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
