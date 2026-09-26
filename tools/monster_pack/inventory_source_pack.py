#!/usr/bin/env py -3
"""Inventory Monsters Full Pack Vol 1 (external Unity source tree)."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

DEFAULT_SOURCE = Path(r"C:\Users\matt1\My project\Assets\Monsters Full Pack Vol 1")
EXCLUDE_NAMES = {"cavecrawler"}


def render_key(name: str) -> str:
    key = re.sub(r"[^a-z0-9]+", "_", name.strip().lower())
    return key.strip("_")


def animation_prefix(fbx_dir: Path, monster_name: str) -> str:
    if not fbx_dir.is_dir():
        return monster_name.replace(" ", "")
    for pattern in (f"{monster_name}@*.FBX", f"{monster_name}@*.fbx"):
        if list(fbx_dir.glob(pattern)):
            return monster_name
    for path in sorted(fbx_dir.glob("*@*.FBX")):
        return path.name.split("@", 1)[0]
    return monster_name.replace(" ", "")


def inventory_monster(mdir: Path, volume: str) -> dict:
    fbx_dir = mdir / "FBX Files"
    fbxs = sorted(fbx_dir.glob("*.FBX")) if fbx_dir.is_dir() else sorted(mdir.rglob("*.FBX"))
    fbxs += sorted(fbx_dir.glob("*.fbx")) if fbx_dir.is_dir() else []
    sk = [f for f in fbxs if f.name.upper().startswith("SK_")]
    anims = [f for f in fbxs if "@" in f.name]
    prefix = animation_prefix(fbx_dir, mdir.name) if fbx_dir.is_dir() else mdir.name.replace(" ", "")
    textures = list(mdir.rglob("*.png")) + list(mdir.rglob("*.tga"))
    return {
        "name": mdir.name,
        "render_key": render_key(mdir.name),
        "volume": volume,
        "source_dir": str(mdir),
        "sk_fbx": str(sk[0]) if sk else None,
        "animation_prefix": prefix,
        "animation_fbx_count": len(anims),
        "animation_fbx_names": [f.name for f in anims],
        "texture_count": len(textures),
        "incomplete": not sk,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--out", type=Path, default=Path(__file__).resolve().parent / "inventory.json")
    args = parser.parse_args()
    if not args.source.is_dir():
        raise SystemExit(f"Source not found: {args.source}")

    monsters: list[dict] = []
    for vol in sorted(args.source.glob("Monsters Pack Vol *")):
        if not vol.is_dir():
            continue
        for mdir in sorted(vol.iterdir()):
            if not mdir.is_dir() or mdir.name.endswith(".meta"):
                continue
            if mdir.name.lower() in EXCLUDE_NAMES:
                continue
            monsters.append(inventory_monster(mdir, vol.name))

    payload = {"source_root": str(args.source), "count": len(monsters), "monsters": monsters}
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    incomplete = [m["name"] for m in monsters if m["incomplete"]]
    print(f"Wrote {args.out} ({len(monsters)} monsters)")
    if incomplete:
        print("INCOMPLETE (no SK_ FBX):", ", ".join(incomplete))


if __name__ == "__main__":
    main()
