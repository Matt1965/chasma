#!/usr/bin/env python3
"""
Monster pack export: one Unity monster folder -> one game-ready GLB.

    py -3 tools/monster_pack/export_monster_fbx2gltf.py --source-dir "<pack>/Monsters Pack Vol 2/Muscomorph" --output assets/units/muscomorph.glb

The folder layout is the Unity pack's own (`FBX Files/SK_<Name>.FBX`, `FBX Files/<Name>@<Clip>.FBX`,
`Prefabs/*.prefab`, `Materials/`, `Textures/`); SK file, clip prefix and prefab are discovered from
it. `--sk-fbx`, `--anim-prefix`, `--prefab` exist only to pick a specific variant.

Steps (all generic, no per-asset branches):
  1. FBX2glTF: SK and every clip FBX -> GLB
  2. glTF merge: copy clip channels verbatim onto the SK node tree (by node name), and
     STEP-fill un-keyed skin joints from each clip file's node defaults when they differ
     from the SK bind (Unity poses clip-rig rest for un-keyed bones; required for jaw/tongue)
  3. skin influences: fold JOINTS_n/WEIGHTS_n into one normalized 4-influence set
  4. timeline: shift clip start to t=0 (timeline repair only, keys untouched)
  5. materials: bind Unity prefab material/textures
  6. validate: skinning invariants + rest-pose deviation off-origin + clip completeness + source agreement
"""

from __future__ import annotations

import argparse
import hashlib
import json
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
from gltf_skin_influences import normalize_skin_influences  # noqa: E402
from unity_material_resolve import resolve_prefab_material  # noqa: E402


class DiscoveryError(Exception):
    pass


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def discover_sk_fbx(fbx_dir: Path) -> Path:
    candidates = sorted(p for p in fbx_dir.iterdir() if p.suffix.lower() == ".fbx" and p.stem.startswith("SK_"))
    if len(candidates) != 1:
        raise DiscoveryError(f"expected exactly one SK_*.FBX in {fbx_dir}, found {[p.name for p in candidates]}; pass --sk-fbx")
    return candidates[0]


def discover_anim_prefix(fbx_dir: Path, sk_fbx: Path) -> str:
    """Clip files are `<Prefix>@<Clip>.FBX`; prefix is the SK stem without `SK_` unless clips say otherwise."""
    prefixes = sorted({p.stem.split("@", 1)[0] for p in fbx_dir.iterdir() if p.suffix.lower() == ".fbx" and "@" in p.stem})
    default = sk_fbx.stem[3:] if sk_fbx.stem.startswith("SK_") else sk_fbx.stem
    if default in prefixes:
        return default
    if len(prefixes) == 1:
        return prefixes[0]
    raise DiscoveryError(f"cannot determine clip prefix in {fbx_dir}: candidates {prefixes}; pass --anim-prefix")


def discover_prefab(source_dir: Path) -> Path | None:
    prefab_dir = source_dir / "Prefabs"
    if not prefab_dir.is_dir():
        return None
    prefabs = sorted(prefab_dir.glob("*.prefab"))
    if not prefabs:
        return None
    if len(prefabs) == 1:
        return prefabs[0]
    base = [p for p in prefabs if "variant" not in p.stem.lower()]
    if len(base) == 1:
        return base[0]
    raise DiscoveryError(f"multiple prefabs in {prefab_dir} ({[p.name for p in prefabs]}); pass --prefab to choose the variant")


def discover_clip_fbx(fbx_dir: Path, anim_prefix: str) -> dict[str, Path]:
    clips: dict[str, Path] = {}
    for fbx in sorted(fbx_dir.iterdir()):
        if fbx.suffix.lower() != ".fbx" or "@" not in fbx.stem:
            continue
        prefix, clip_name = fbx.stem.split("@", 1)
        if prefix != anim_prefix or "|" in clip_name or "Take 001" in clip_name:
            continue
        clips.setdefault(clip_name, fbx)
    return clips


def export_monster(
    source_dir: Path,
    output_glb: Path,
    *,
    sk_fbx: Path | None = None,
    anim_prefix: str | None = None,
    prefab: Path | None = None,
    include_rm: bool = True,
    scratch: Path | None = None,
) -> dict:
    fbx_dir = source_dir / "FBX Files"
    if not fbx_dir.is_dir():
        raise FileNotFoundError(f"Missing FBX Files: {fbx_dir}")
    sk_fbx = sk_fbx or discover_sk_fbx(fbx_dir)
    anim_prefix = anim_prefix or discover_anim_prefix(fbx_dir, sk_fbx)
    prefab = prefab or discover_prefab(source_dir)
    clip_fbx = discover_clip_fbx(fbx_dir, anim_prefix)
    if not include_rm:
        clip_fbx = {k: v for k, v in clip_fbx.items() if not k.endswith("_RM")}
    if not clip_fbx:
        raise FileNotFoundError(f"No clip FBX for prefix {anim_prefix} in {fbx_dir}")

    work = scratch or Path(tempfile.mkdtemp(prefix="chasma_monster_export_"))
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

    gltf, blob = load_glb(merged)
    blob, influence_report = normalize_skin_influences(gltf, blob)
    blob, rebased = rebase_gltf_animation_starts_to_zero(gltf, blob)
    write_glb(merged, gltf, blob)

    mat_resolution = resolve_prefab_material(source_dir, prefab)
    if mat_resolution.ambiguous:
        raise RuntimeError(f"material resolution failed: {mat_resolution.ambiguity_detail}")
    if mat_resolution.resolved is None:
        raise RuntimeError("material resolution produced no material")
    mats = mat_resolution.resolved_materials or [mat_resolution.resolved]
    material_report = apply_materials(merged, mats, merged)

    validation = validate_export(merged, sorted(clip_glbs.keys()), clip_glbs)
    if not validation["complete"]:
        raise RuntimeError("validation failed: " + "; ".join(validation["errors"]))

    output_glb.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(merged, output_glb)

    manifest = {
        "pipeline": "fbx2gltf_gltf_merge",
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "source_dir": str(source_dir),
        "sk_fbx": str(sk_fbx),
        "anim_prefix": anim_prefix,
        "output_glb": str(output_glb),
        "output_bytes": output_glb.stat().st_size,
        "output_sha256": sha256_file(output_glb),
        "fbx2gltf": {"exe": str(resolve_fbx2gltf_exe()), "sk": sk_conv, "clips": clip_conversions},
        "merge": merge_report,
        "skin_influences": influence_report,
        "rebased_clips": rebased,
        "material": {
            "prefab": str(mat_resolution.prefab_path),
            "material_guids": mat_resolution.renderer_material_guids,
            "apply": material_report,
        },
        "validation": validation,
        "complete": True,
    }
    output_glb.with_suffix(".export_manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return manifest


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--source-dir", type=Path, required=True, help="Unity monster folder (contains `FBX Files/`)")
    ap.add_argument("--output", type=Path, required=True, help="destination GLB, e.g. assets/units/<name>.glb")
    ap.add_argument("--sk-fbx", type=Path, default=None, help="override: skinned mesh FBX")
    ap.add_argument("--anim-prefix", default=None, help="override: clip file prefix before `@`")
    ap.add_argument("--prefab", type=Path, default=None, help="override: prefab (material variant)")
    ap.add_argument("--no-rm-clips", action="store_true", help="skip *_RM root-motion clip variants")
    args = ap.parse_args()
    manifest = export_monster(
        args.source_dir,
        args.output,
        sk_fbx=args.sk_fbx,
        anim_prefix=args.anim_prefix,
        prefab=args.prefab,
        include_rm=not args.no_rm_clips,
    )
    summary = {
        "complete": manifest["complete"],
        "output": manifest["output_glb"],
        "bytes": manifest["output_bytes"],
        "clips": manifest["validation"]["clips"]["have_count"],
        "translation_invariance_error_m": manifest["validation"]["translation_invariance"]["max_error_m"],
        "rest_vs_mesh_node_max_dev_m": manifest["validation"]["rest_skinning"]["max_dev_m"],
        "skin_influence_sets_folded": manifest["skin_influences"]["max_input_sets"],
        "warnings": manifest["validation"]["warnings"],
    }
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
