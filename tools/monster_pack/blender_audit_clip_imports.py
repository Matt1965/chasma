#!/usr/bin/env python3
"""Audit per-clip FBX import outcomes (headless Blender)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import bpy

_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

from monster_export_core import FBX_SCALE, clip_name_from_fbx, import_fbx  # noqa: E402


def parse_args() -> tuple[Path, str, Path]:
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1 :]
    else:
        argv = []
    if len(argv) < 3:
        raise SystemExit("usage: blender --background --python blender_audit_clip_imports.py -- <fbx_dir> <prefix> <out.json>")
    return Path(argv[0]), argv[1], Path(argv[2])


def clear_scene() -> None:
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete()
    for action in list(bpy.data.actions):
        bpy.data.actions.remove(action)


def main() -> None:
    fbx_dir, prefix, out_path = parse_args()
    paths = sorted(fbx_dir.glob(f"{prefix}@*.FBX")) + sorted(fbx_dir.glob(f"{prefix}@*.fbx"))
    rows: list[dict] = []
    for path in paths:
        clip = clip_name_from_fbx(prefix, path.name)
        clear_scene()
        error = None
        action_name = None
        try:
            import_fbx(path)
            actions = [a for a in bpy.data.actions if a]
            if actions:
                action_name = actions[0].name
            else:
                error = "no_action_after_import"
        except Exception as exc:  # noqa: BLE001 — audit records all importer failures
            error = f"{type(exc).__name__}: {exc}"
        rows.append(
            {
                "source_file": path.name,
                "clip_name": clip,
                "import_ok": error is None,
                "blender_action": action_name,
                "error": error,
            }
        )

    unique_clips = {r["clip_name"] for r in rows if r["import_ok"]}
    report = {
        "fbx_dir": str(fbx_dir),
        "animation_prefix": prefix,
        "source_files": len(paths),
        "unique_clips_ok": len(unique_clips),
        "rows": rows,
    }
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(f"Wrote {out_path} ({len(paths)} files, {len(unique_clips)} unique clips ok)")


if __name__ == "__main__":
    main()
