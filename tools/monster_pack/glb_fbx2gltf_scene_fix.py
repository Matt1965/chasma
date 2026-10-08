#!/usr/bin/env python3
"""Documented GLB repairs for Godot FBX2glTF -> Blender glTF monsters (no bind hacks)."""

from __future__ import annotations

from pathlib import Path

from glb_sanitize_animations import SCENE_ROOT_ROTATION_X90, load_glb, write_glb
from bufomorph_glb_helpers import (
    print_recommended_model_offset_y,
    validate_bufomorph_idle_jaw_timing,
)
from monster_orientation import verify_presentation_upright

_BUFOMORPH_BODY_BONE = "Bufomorph_"
_BUFOMORPH_CATALOG_YAW_DEG = 180.0


def apply_fbx2gltf_scene_root_x90(path: Path) -> str:
    """Match direct-FBX exports: +90 deg X on scene root (FBX2glTF drops armature object rotation)."""
    gltf, blob = load_glb(path)
    scene = gltf["scenes"][0]
    root_index = scene["nodes"][0]
    gltf["nodes"][root_index]["rotation"] = list(SCENE_ROOT_ROTATION_X90)
    write_glb(path, gltf, blob)
    return "scene_root_x90"


def clear_scene_root_rotation(path: Path) -> str:
    """Remove scene-root rotation when armature root already carries FBX orientation."""
    gltf, blob = load_glb(path)
    scene = gltf["scenes"][0]
    root_index = scene["nodes"][0]
    gltf["nodes"][root_index].pop("rotation", None)
    write_glb(path, gltf, blob)
    return "scene_root_identity"


def diagnose_bufomorph_glb(path: Path) -> None:
    """Read-only checks after faithful export (does not modify the GLB)."""
    print(path, validate_bufomorph_idle_jaw_timing(path))
    ok, up = verify_presentation_upright(
        path,
        _BUFOMORPH_BODY_BONE,
        catalog_yaw_y_deg=_BUFOMORPH_CATALOG_YAW_DEG,
        catalog_roll_z_deg=90.0,
    )
    print(path, f"verify_presentation_upright ok={ok} up={tuple(round(c, 3) for c in up)}")
    print(
        path,
        print_recommended_model_offset_y(
            path, _BUFOMORPH_BODY_BONE, _BUFOMORPH_CATALOG_YAW_DEG
        ),
    )


if __name__ == "__main__":
    import sys

    for arg in sys.argv[1:]:
        diagnose_bufomorph_glb(Path(arg))
