"""FBX2glTF invocation with pinned settings (Option B skeleton/clip path)."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

FBX2GLTF_VERSION = "0.13.1"
FBX2GLTF_CANDIDATES = (
    Path(__file__).resolve().parent / "bin" / "FBX2glTF-windows-x86_64.exe",
    Path(r"C:\BevyFiles\Temp\FBX2glTF-win\FBX2glTF-windows-x86_64\FBX2glTF-windows-x86_64.exe"),
)


def resolve_fbx2gltf_exe() -> Path:
    env = os.environ.get("MONSTER_FBX2GLTF_EXE")
    if env:
        path = Path(env)
        if path.is_file():
            return path
    for path in FBX2GLTF_CANDIDATES:
        if path.is_file():
            return path
    raise FileNotFoundError(
        "FBX2glTF not found. Run tools/monster_pack/ensure_fbx2gltf.ps1 or set MONSTER_FBX2GLTF_EXE."
    )


def convert_fbx_to_glb(fbx_path: Path, output_glb: Path) -> dict:
    """Convert one FBX to GLB. Output stem must not include .glb (FBX2glTF appends it)."""
    exe = resolve_fbx2gltf_exe()
    output_glb.parent.mkdir(parents=True, exist_ok=True)
    out_stem = output_glb.with_suffix("")
    cmd = [str(exe), "--input", str(fbx_path), "--output", str(out_stem), "--binary"]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(
            f"FBX2glTF failed for {fbx_path.name} (code {proc.returncode}):\n{proc.stdout}\n{proc.stderr}"
        )
    if not output_glb.is_file():
        raise RuntimeError(f"FBX2glTF did not write {output_glb}")
    return {
        "fbx": str(fbx_path),
        "glb": str(output_glb),
        "fbx2gltf_exe": str(exe),
        "fbx2gltf_version": FBX2GLTF_VERSION,
        "command": cmd,
    }
