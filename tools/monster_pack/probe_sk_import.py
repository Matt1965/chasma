#!/usr/bin/env py -3
"""Probe which SK_* FBX files import in Blender."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BLENDER = Path(r"C:\BevyFiles\Temp\blender-4.2.3\blender-4.2.3-windows-x64\blender.exe")


def probe(sk: Path) -> bool:
    code = f"""
import bpy
try:
    bpy.ops.import_scene.fbx(filepath=r"{sk.as_posix()}", global_scale=0.01, use_anim=False)
    print("OK")
except Exception as exc:
    print("FAIL", exc)
"""
    result = subprocess.run(
        [str(BLENDER), "--background", "--python-expr", code],
        capture_output=True,
        text=True,
        timeout=180,
    )
    return "OK" in result.stdout and "FAIL" not in result.stdout


def main() -> None:
    inv = json.loads((ROOT / "inventory.json").read_text(encoding="utf-8"))
    ok: list[str] = []
    bad: list[tuple[str, str]] = []
    for monster in inv["monsters"]:
        sk_raw = monster.get("sk_fbx")
        sk = Path(sk_raw) if sk_raw else Path(monster["source_dir"]) / "FBX Files" / f"SK_{monster['name']}.FBX"
        if not sk.is_file():
            bad.append((monster["name"], "missing SK"))
            continue
        if probe(sk):
            ok.append(monster["name"])
        else:
            bad.append((monster["name"], "blender import error"))
    print(json.dumps({"ok": ok, "bad": bad}, indent=2))


if __name__ == "__main__":
    main()
