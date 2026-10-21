#!/usr/bin/env py -3
"""Rebuild import_manifest.json from inventory + exported GLBs."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
INVENTORY = ROOT / "inventory.json"
MANIFEST = ROOT / "import_manifest.json"
GLB_CLIPS = ROOT / "glb_clips.py"
UNITS_DIR = REPO / "assets" / "units"


def main() -> None:
    inv = json.loads(INVENTORY.read_text(encoding="utf-8"))
    manifest: list[dict] = []
    for monster in inv["monsters"]:
        key = monster["render_key"]
        glb = UNITS_DIR / f"{key}.glb"
        if not glb.is_file():
            continue
        result = subprocess.run(
            ["py", "-3", str(GLB_CLIPS), str(glb)],
            capture_output=True,
            text=True,
            check=True,
        )
        clips = [line.strip() for line in result.stdout.splitlines() if line.strip()]
        manifest.append(
            {
                "monster": monster["name"],
                "render_key": key,
                "glb_path": f"assets\\units\\{key}.glb",
                "clips": clips,
            }
        )
    MANIFEST.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    print(f"Wrote {MANIFEST} ({len(manifest)} monsters)")


if __name__ == "__main__":
    main()
