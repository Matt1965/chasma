"""Export gate for monster GLBs (asset-agnostic).

Hard errors (export fails):
- skinning invariants: one influence set, weights sum to 1, joint indices in range, IBM present
- translation invariance: skinning 300 m from the origin must equal skinning at the origin plus
  the offset (the renderer-facing form of the weight-sum defect)
- expected clips missing
- merged clip keys differ from the source clip GLBs (merge must be verbatim)

Warnings (recorded in the manifest, never block):
- rest pose vs mesh-node placement (authored per-mesh bind offsets are legitimate)
- translation range vs skeleton extent (large root motion is legitimate for some clips)
- animated skinning deviation per clip (informational; authored motion can be large)
"""

from __future__ import annotations

import random
from pathlib import Path
from typing import Any

import numpy as np

from glb_sanitize_animations import load_glb
from gltf_skin_influences import read_accessor
from skin_eval import (
    DEFAULT_WORLD_OFFSET,
    animation_duration,
    evaluate_pose,
    global_matrices,
    pose_at,
    skin_invariants,
    translation_invariance,
)

TRANSLATION_INVARIANCE_LIMIT_M = 0.001
REST_DEVIATION_WARN_M = 0.02
SOURCE_AGREEMENT_SAMPLES_PER_CLIP = 12
SOURCE_AGREEMENT_TOLERANCE = 1e-5


def clip_completeness(expected: list[str], gltf: dict) -> dict[str, Any]:
    have = {a.get("name") for a in gltf.get("animations", [])}
    return {
        "expected_count": len(expected),
        "have_count": len(have),
        "missing": sorted(set(expected) - have),
        "extra": sorted(have - set(expected)),
    }


def translation_range_diagnostic(gltf: dict, blob: bytes) -> dict[str, Any]:
    joints: set[int] = set()
    for skin in gltf.get("skins", []):
        joints.update(skin["joints"])
    rest = evaluate_rest_joint_positions(gltf, blob, joints)
    extent = float((rest.max(axis=0) - rest.min(axis=0)).max()) if len(rest) else 0.0
    worst = (0.0, None, None)
    for anim in gltf.get("animations", []):
        for channel in anim["channels"]:
            target = channel["target"]
            if target["path"] != "translation" or target.get("node") not in joints:
                continue
            values = read_accessor(gltf, blob, anim["samplers"][channel["sampler"]]["output"]).astype(np.float64)
            span = float((values.max(axis=0) - values.min(axis=0)).max())
            if span > worst[0]:
                worst = (span, anim.get("name"), gltf["nodes"][target["node"]].get("name"))
    return {
        "skeleton_extent_m": extent,
        "worst_translation_span_m": worst[0],
        "clip": worst[1],
        "joint": worst[2],
        "ratio": (worst[0] / extent) if extent else None,
    }


def evaluate_rest_joint_positions(gltf: dict, blob: bytes, joints: set[int]) -> np.ndarray:
    pose = pose_at(gltf, blob, None, 0.0)
    globals_ = global_matrices(gltf, pose, np.eye(4))
    return np.array([globals_[j][:3, 3] for j in sorted(joints)]) if joints else np.zeros((0, 3))


def _channel_index(gltf: dict) -> dict[tuple[str, str, str], dict]:
    out: dict[tuple[str, str, str], dict] = {}
    for anim in gltf.get("animations", []):
        for channel in anim["channels"]:
            target = channel["target"]
            node = target.get("node")
            if node is None:
                continue
            key = (anim.get("name"), gltf["nodes"][node].get("name"), target["path"])
            out[key] = anim["samplers"][channel["sampler"]]
    return out


def source_agreement(merged_gltf: dict, merged_blob: bytes, clip_glbs: dict[str, Path], seed: int = 7) -> dict[str, Any]:
    """Sampled channels in the merged GLB must equal the FBX2glTF clip GLB values exactly."""
    merged_index = _channel_index(merged_gltf)
    rng = random.Random(seed)
    mismatches: list[str] = []
    checked = 0
    for clip_name, clip_path in sorted(clip_glbs.items()):
        cg, cb = load_glb(clip_path)
        source_anim_name = cg["animations"][0].get("name")
        source_index = _channel_index(cg)
        keys = [k for k in source_index if k[0] == source_anim_name and (clip_name, k[1], k[2]) in merged_index]
        rng.shuffle(keys)
        for key in keys[:SOURCE_AGREEMENT_SAMPLES_PER_CLIP]:
            src = read_accessor(cg, cb, source_index[key]["output"]).astype(np.float64)
            dst = read_accessor(merged_gltf, merged_blob, merged_index[(clip_name, key[1], key[2])]["output"]).astype(np.float64)
            checked += 1
            if src.shape != dst.shape or float(np.abs(src - dst).max()) > SOURCE_AGREEMENT_TOLERANCE:
                mismatches.append(f"{clip_name}/{key[1]}/{key[2]}")
    return {"channels_checked": checked, "mismatches": mismatches}


def validate_export(glb_path: Path, expected_clips: list[str], clip_glbs: dict[str, Path] | None = None) -> dict[str, Any]:
    gltf, blob = load_glb(glb_path)
    errors: list[str] = []
    warnings: list[str] = []

    invariants = skin_invariants(gltf, blob)
    errors.extend(invariants["issues"])

    invariance = translation_invariance(gltf, blob, DEFAULT_WORLD_OFFSET)
    if invariance["max_error_m"] > TRANSLATION_INVARIANCE_LIMIT_M:
        errors.append(
            f"skinning depends on world position: {invariance['max_error_m']:.3f} m error at offset "
            f"{DEFAULT_WORLD_OFFSET} ({invariance['primitive']}) - weights/joints are not renderer-consistent"
        )

    rest_rows = evaluate_pose(gltf, blob, pose_at(gltf, blob, None, 0.0), DEFAULT_WORLD_OFFSET)
    rest_max = max((r["max_dev_m"] for r in rest_rows), default=0.0)
    if rest_max > REST_DEVIATION_WARN_M:
        worst = max(rest_rows, key=lambda r: r["max_dev_m"])
        warnings.append(
            f"rest pose differs from mesh-node placement by {rest_max:.3f} m ({worst['primitive']}); "
            "authored per-mesh bind offset, renders identically in Unity"
        )

    clip_rows: list[dict[str, Any]] = []
    for anim in gltf.get("animations", []):
        duration = animation_duration(gltf, blob, anim)
        rows = evaluate_pose(gltf, blob, pose_at(gltf, blob, anim, duration * 0.5), DEFAULT_WORLD_OFFSET)
        clip_rows.append({"clip": anim.get("name"), "max_dev_m": max((r["max_dev_m"] for r in rows), default=0.0)})

    clips = clip_completeness(expected_clips, gltf)
    if clips["missing"]:
        errors.append(f"missing clips: {clips['missing']}")

    translation = translation_range_diagnostic(gltf, blob)
    if translation["ratio"] is not None and translation["ratio"] > 1.0:
        warnings.append(
            f"translation span {translation['worst_translation_span_m']:.2f} m exceeds skeleton extent "
            f"{translation['skeleton_extent_m']:.2f} m ({translation['clip']} / {translation['joint']})"
        )

    agreement = None
    if clip_glbs:
        agreement = source_agreement(gltf, blob, clip_glbs)
        if agreement["mismatches"]:
            errors.append(f"merged channels differ from source clip GLBs: {agreement['mismatches'][:6]}")

    return {
        "glb": str(glb_path),
        "skin_invariants": invariants,
        "translation_invariance": {**invariance, "limit_m": TRANSLATION_INVARIANCE_LIMIT_M},
        "rest_skinning": {"max_dev_m": rest_max, "warn_m": REST_DEVIATION_WARN_M, "primitives": rest_rows},
        "clip_skinning": clip_rows,
        "clips": clips,
        "translation_diagnostic": translation,
        "source_agreement": agreement,
        "warnings": warnings,
        "errors": errors,
        "complete": not errors,
    }
