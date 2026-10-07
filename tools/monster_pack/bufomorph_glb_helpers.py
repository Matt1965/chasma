#!/usr/bin/env python3
"""Bufomorph GLB helpers (ground sink only — do not hack jaw/skin bind)."""

from __future__ import annotations

from pathlib import Path

from glb_sanitize_animations import load_glb, write_glb
from monster_orientation import (
    IDENTITY_QUAT,
    catalog_correction_quat,
    parent_map,
    quat_mul,
    quat_rotate,
    world_rotation,
)

import struct

_JAW_UPPER = "Bufomorph_JawUpper"


def validate_bufomorph_idle_jaw_timing(path: Path) -> str:
    """
    Guardrail: jaw track must span the idle clip (catches Death-pasted jaw on locomotion).
    Expect IdleBreathe ~2s at t=0 after NLA rebase in glb_sanitize_animations.
    Does not modify the GLB.
    """
    gltf, blob = load_glb(path)
    idle = next((a for a in gltf.get("animations", []) if a.get("name") == "IdleBreathe"), None)
    if idle is None:
        raise SystemExit(f"{path}: missing IdleBreathe animation")

    clip_start = 1e9
    clip_end = 0.0
    for channel in idle.get("channels", []):
        sampler = idle["samplers"][channel["sampler"]]
        input_acc = gltf["accessors"][sampler["input"]]
        bv = gltf["bufferViews"][input_acc["bufferView"]]
        off = bv.get("byteOffset", 0) + input_acc.get("byteOffset", 0)
        start_t = struct.unpack_from("<f", blob, off)[0]
        end_t = struct.unpack_from("<f", blob, off + (input_acc["count"] - 1) * 4)[0]
        clip_start = min(clip_start, start_t)
        clip_end = max(clip_end, end_t)

    if clip_start > 0.05:
        raise SystemExit(
            f"{path}: IdleBreathe starts at {clip_start:.2f}s — run sanitize/rebase "
            "(NLA export timeline leak); expected t=0."
        )

    jaw_end = None
    for channel in idle.get("channels", []):
        target = channel.get("target", {})
        if target.get("path") != "rotation":
            continue
        node_index = target.get("node")
        if (gltf["nodes"][node_index].get("name") or "").strip() != _JAW_UPPER:
            continue
        sampler = idle["samplers"][channel["sampler"]]
        input_acc = gltf["accessors"][sampler["input"]]
        bv = gltf["bufferViews"][input_acc["bufferView"]]
        off = bv.get("byteOffset", 0) + input_acc.get("byteOffset", 0)
        jaw_end = struct.unpack_from("<f", blob, off + (input_acc["count"] - 1) * 4)[0]
        break

    if jaw_end is None:
        raise SystemExit(f"{path}: IdleBreathe missing {_JAW_UPPER} rotation channel")

    if jaw_end < clip_end - 0.2:
        raise SystemExit(
            f"{path}: IdleBreathe jaw track ends at {jaw_end:.2f}s (clip ~{clip_end:.2f}s) — "
            "re-export bufomorph.glb from the pack; do not patch jaw data in the fix script."
        )
    if clip_end > 6.0:
        raise SystemExit(
            f"{path}: IdleBreathe duration {clip_end:.2f}s looks like unstripped NLA timeline; "
            "re-run export sanitize (rebase animation starts to zero)."
        )
    return f"validate_idle_jaw_timing ok jaw_end={jaw_end:.2f}s clip_end={clip_end:.2f}s"


def _world_translation(
    gltf: dict, idx: int, parents: dict[int, int], cache: dict[int, list[float]]
) -> list[float]:
    if idx in cache:
        return cache[idx]
    node = gltf["nodes"][idx]
    local = list(node.get("translation", [0.0, 0.0, 0.0]))
    if idx not in parents:
        cache[idx] = local
        return local
    parent = parents[idx]
    parent_t = _world_translation(gltf, parent, parents, cache)
    parent_r = world_rotation(gltf, parent, parents, {})
    rotated = quat_rotate(parent_r, local)
    world = [parent_t[i] + rotated[i] for i in range(3)]
    cache[idx] = world
    return world


def estimate_model_offset_y_meters(
    gltf: dict,
    body_bone_name: str,
    catalog_yaw_y_deg: float = 180.0,
) -> float:
    parents = parent_map(gltf)
    scene_index = gltf["scenes"][0]["nodes"][0]
    t_cache: dict[int, list[float]] = {}
    scene_t = _world_translation(gltf, scene_index, parents, t_cache)
    scene_r = world_rotation(gltf, scene_index, parents, {})
    catalog = catalog_correction_quat(0.0, catalog_yaw_y_deg, 0.0)
    visual_rot = quat_mul(catalog, scene_r)

    min_y = 1e9
    for accessor in gltf.get("accessors", []):
        if accessor.get("type") != "VEC3" or "min" not in accessor:
            continue
        mn = accessor["min"]
        mx = accessor.get("max", mn)
        corners = [
            (mn[0], mn[1], mn[2]),
            (mn[0], mn[1], mx[2]),
            (mn[0], mx[1], mn[2]),
            (mn[0], mx[1], mx[2]),
            (mx[0], mn[1], mn[2]),
            (mx[0], mn[1], mx[2]),
            (mx[0], mx[1], mn[2]),
            (mx[0], mx[1], mx[2]),
        ]
        for corner in corners:
            rotated = quat_rotate(visual_rot, list(corner))
            world = [scene_t[i] + rotated[i] for i in range(3)]
            min_y = min(min_y, world[1])

    if min_y > 1e8:
        return 0.0
    return round(0.02 - min_y, 3)


def estimate_scene_sink_translation_y(gltf: dict) -> float:
    parents = parent_map(gltf)
    scene_index = gltf["scenes"][0]["nodes"][0]
    t_cache: dict[int, list[float]] = {}
    scene_t = _world_translation(gltf, scene_index, parents, t_cache)
    scene_r = world_rotation(gltf, scene_index, parents, {})

    min_y = 1e9
    for accessor in gltf.get("accessors", []):
        if accessor.get("type") != "VEC3" or "min" not in accessor:
            continue
        mn, mx = accessor["min"], accessor.get("max", accessor["min"])
        for corner in (
            (mn[0], mn[1], mn[2]),
            (mx[0], mn[1], mn[2]),
            (mn[0], mx[1], mn[2]),
            (mx[0], mx[1], mn[2]),
        ):
            rotated = quat_rotate(scene_r, list(corner))
            world = [scene_t[i] + rotated[i] for i in range(3)]
            min_y = min(min_y, world[1])

    if min_y > 1e8:
        return 0.0
    return round(0.02 - min_y, 3)


def sink_bufomorph_scene_to_ground(path: Path, scene_sink_y: float) -> str:
    gltf, blob = load_glb(path)
    scene_index = gltf["scenes"][0]["nodes"][0]
    scene = gltf["nodes"][scene_index]
    translation = list(scene.get("translation", [0.0, 0.0, 0.0]))
    translation[1] = 0.0
    scene["translation"] = translation
    auto_delta = estimate_scene_sink_translation_y(gltf)
    translation[1] = scene_sink_y
    scene["translation"] = translation
    write_glb(path, gltf, blob)
    return f"scene_sink_y delta={scene_sink_y} auto={auto_delta}"


def print_recommended_model_offset_y(path: Path, body_bone_name: str, catalog_yaw_y_deg: float) -> str:
    gltf, _ = load_glb(path)
    offset_y = estimate_model_offset_y_meters(gltf, body_bone_name, catalog_yaw_y_deg)
    sink_y = estimate_scene_sink_translation_y(gltf)
    return f"recommended_model_offset_y_m={offset_y} scene_sink_y={sink_y}"
