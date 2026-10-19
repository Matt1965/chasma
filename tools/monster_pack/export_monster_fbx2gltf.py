#!/usr/bin/env python3
"""
Option B monster export: FBX2glTF (SK + clips) -> glTF-level merge -> Unity material bind -> validate.

Preserves all clip channels (including *_RM). Does not strip, clamp, or apply scene-root hacks.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

from export_validate import validate_export  # noqa: E402
from fbx2gltf_util import convert_fbx_to_glb, resolve_fbx2gltf_exe  # noqa: E402
from glb_sanitize_animations import load_glb, rebase_gltf_animation_starts_to_zero, write_glb  # noqa: E402
from gltf_material_apply import apply_materials  # noqa: E402
from gltf_merge import merge_clips_into_sk  # noqa: E402
from gltf_unify_skin import unify_glb_skin  # noqa: E402
from unity_material_resolve import resolve_prefab_material  # noqa: E402


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def discover_clip_fbx(fbx_dir: Path, anim_prefix: str) -> dict[str, Path]:
    clips: dict[str, Path] = {}
    patterns = [f"{anim_prefix}@*.FBX", f"{anim_prefix}@*.fbx"]
    for pattern in patterns:
        for fbx in sorted(fbx_dir.glob(pattern)):
            if "@" not in fbx.stem:
                continue
            clip_name = fbx.stem.split("@", 1)[1]
            if "|" in clip_name or "Take 001" in clip_name:
                continue
            clips.setdefault(clip_name, fbx)
    return clips


def export_monster(
    source_dir: Path,
    sk_fbx: Path,
    anim_prefix: str,
    output_glb: Path,
    *,
    prefab: Path | None = None,
    include_rm: bool = True,
    scratch: Path | None = None,
) -> dict:
    fbx_dir = source_dir / "FBX Files"
    if not fbx_dir.is_dir():
        raise FileNotFoundError(f"Missing FBX Files: {fbx_dir}")
    clip_fbx = discover_clip_fbx(fbx_dir, anim_prefix)
    if not include_rm:
        clip_fbx = {k: v for k, v in clip_fbx.items() if not k.endswith("_RM")}
    if not clip_fbx:
        raise FileNotFoundError(f"No clip FBX for prefix {anim_prefix}")

    work = scratch or Path(tempfile.mkdtemp(prefix="chasma_option_b_"))
    work.mkdir(parents=True, exist_ok=True)

    sk_glb = work / sk_fbx.with_suffix(".glb").name
    sk_conv = convert_fbx_to_glb(sk_fbx, sk_glb)

    clip_glbs: dict[str, Path] = {}
    clip_conversions = []
    for clip_name, fbx_path in sorted(clip_fbx.items()):
        out = work / f"{anim_prefix}@{clip_name}.glb"
        clip_conversions.append({"clip": clip_name, **convert_fbx_to_glb(fbx_path, out)})
        clip_glbs[clip_name] = out

    merged = work / "merged.glb"
    merge_report = merge_clips_into_sk(sk_glb, clip_glbs, merged)
    skin_report = unify_glb_skin(merged)
    merge_report["skin_unify"] = skin_report

    gltf, blob = load_glb(merged)
    blob, rebased = rebase_gltf_animation_starts_to_zero(gltf, blob)
    write_glb(merged, gltf, blob)

    mat_resolution = resolve_prefab_material(source_dir, prefab)
    material_report = None
    if mat_resolution.ambiguous:
        raise RuntimeError(f"material resolution failed: {mat_resolution.ambiguity_detail}")
    if mat_resolution.resolved is None:
        raise RuntimeError("material resolution produced no material")
    mats = mat_resolution.resolved_materials or ([mat_resolution.resolved] if mat_resolution.resolved else [])
    material_report = apply_materials(merged, mats, merged)

    if output_glb.is_file():
        backup = output_glb.with_suffix(output_glb.suffix + ".pre_option_b_backup")
        shutil.copy2(output_glb, backup)

    output_glb.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(merged, output_glb)

    validation = validate_export(output_glb, sorted(clip_glbs.keys()), clip_glbs)
    if not validation["complete"]:
        raise RuntimeError("validation failed: " + "; ".join(validation["errors"]))

    manifest = {
        "pipeline": "option_b_fbx2gltf_gltf_merge",
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "source_dir": str(source_dir),
        "sk_fbx": str(sk_fbx),
        "anim_prefix": anim_prefix,
        "output_glb": str(output_glb),
        "output_bytes": output_glb.stat().st_size,
        "output_sha256": sha256_file(output_glb),
        "fbx2gltf": {
            "exe": str(resolve_fbx2gltf_exe()),
            "sk": sk_conv,
            "clips": clip_conversions,
        },
        "merge": merge_report,
        "rebased_clips": rebased,
        "material": {
            "prefab": str(mat_resolution.prefab_path),
            "material_guids": mat_resolution.renderer_material_guids,
            "apply": material_report,
        },
        "validation": validation,
        "complete": True,
    }
    manifest_path = output_glb.with_suffix(".export_manifest.json")
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return manifest


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--source-dir", type=Path, required=True)
    ap.add_argument("--sk-fbx", type=Path, required=True)
    ap.add_argument("--anim-prefix", required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--prefab", type=Path, default=None)
    ap.add_argument("--no-rm-clips", action="store_true")
    args = ap.parse_args()
    manifest = export_monster(
        args.source_dir,
        args.sk_fbx,
        args.anim_prefix,
        args.output,
        prefab=args.prefab,
        include_rm=not args.no_rm_clips,
    )
    print(json.dumps({"complete": manifest["complete"], "output": manifest["output_glb"], "bytes": manifest["output_bytes"]}, indent=2))


if __name__ == "__main__":
    main()
