#!/usr/bin/env python3
"""
Headless Blender renders of a skinned GLB at specific clip times.

Outer object rotation is applied for framing only (not internal bones).
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Euler, Vector


def parse_args() -> argparse.Namespace:
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1 :]
    else:
        argv = []
    parser = argparse.ArgumentParser()
    parser.add_argument("--glb", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--label", required=True, help="Prefix for output PNG names")
    parser.add_argument(
        "--outer-rotation-deg",
        default="0,0,0",
        help="Euler XYZ degrees on imported root for framing (sideways models)",
    )
    parser.add_argument("--clip", action="append", default=[], help="clip=time_seconds (repeatable)")
    parser.add_argument("--resolution", type=int, default=512)
    return parser.parse_args(argv)


def clear_scene() -> None:
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete()
    for action in list(bpy.data.actions):
        bpy.data.actions.remove(action)


def find_armature() -> bpy.types.Object | None:
    for obj in bpy.data.objects:
        if obj.type == "ARMATURE":
            return obj
    return None


def setup_render(resolution: int) -> None:
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE_NEXT"
    scene.render.resolution_x = resolution
    scene.render.resolution_y = resolution
    scene.render.film_transparent = False
    scene.world.use_nodes = True
    bg = scene.world.node_tree.nodes["Background"]
    bg.inputs[0].default_value = (0.55, 0.58, 0.62, 1.0)
    bg.inputs[1].default_value = 1.0

    bpy.ops.object.light_add(type="SUN", location=(4.0, -3.0, 6.0))
    sun = bpy.context.active_object
    sun.data.energy = 3.0
    bpy.ops.object.light_add(type="AREA", location=(-3.0, 4.0, 2.0))
    area = bpy.context.active_object
    area.data.energy = 250.0
    area.data.size = 4.0


def frame_armature(armature: bpy.types.Object, camera: bpy.types.Object) -> None:
    bpy.context.view_layer.update()
    corners = [armature.matrix_world @ Vector(corner) for corner in armature.bound_box]
    min_v = Vector((min(c[i] for c in corners) for i in range(3)))
    max_v = Vector((max(c[i] for c in corners) for i in range(3)))
    center = (min_v + max_v) * 0.5
    size = max((max_v - min_v).length, 0.5)
    # Head/jaw bias: shift target slightly toward +Z in armature space for mouth visibility.
    target = center + Vector((0.0, size * 0.15, size * 0.2))
    camera.location = target + Vector((size * 1.4, -size * 1.8, size * 0.55))
    direction = target - camera.location
    camera.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()


def resolve_action(armature: bpy.types.Object, clip_name: str) -> bpy.types.Action:
    action = bpy.data.actions.get(clip_name)
    if action is not None:
        return action
    for candidate in bpy.data.actions:
        if candidate.name == clip_name or candidate.name.endswith(clip_name):
            return candidate
    if armature.animation_data:
        for track in armature.animation_data.nla_tracks:
            if track.name == clip_name and track.strips:
                return track.strips[0].action
    names = [a.name for a in bpy.data.actions]
    raise SystemExit(f"clip {clip_name} not found (actions={names})")


def apply_clip_time(armature: bpy.types.Object, clip_name: str, time_s: float) -> None:
    if armature.animation_data is None:
        armature.animation_data_create()
    action = resolve_action(armature, clip_name)
    armature.animation_data.action = action
    frame = int(round(time_s * bpy.context.scene.render.fps))
    bpy.context.scene.frame_set(frame)
    bpy.context.view_layer.update()


def main() -> None:
    args = parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)
    clips = []
    for item in args.clip or ["IdleBreathe=0.0", "IdleBreathe=1.0", "BiteAttack=0.35"]:
        name, t_s = item.split("=", 1)
        clips.append((name, float(t_s)))

    rx, ry, rz = (float(v) for v in args.outer_rotation_deg.split(","))
    outer_euler = Euler((math.radians(rx), math.radians(ry), math.radians(rz)), "XYZ")

    clear_scene()
    setup_render(args.resolution)
    bpy.ops.import_scene.gltf(filepath=str(args.glb))
    armature = find_armature()
    if armature is None:
        raise SystemExit(f"No armature in {args.glb}")

    wrapper = bpy.data.objects.new("PresentationWrapper", None)
    bpy.context.collection.objects.link(wrapper)
    armature.parent = wrapper
    wrapper.rotation_euler = outer_euler

    bpy.ops.object.camera_add()
    camera = bpy.context.active_object
    bpy.context.scene.camera = camera

    manifest = {
        "glb": str(args.glb.resolve()),
        "label": args.label,
        "outer_rotation_deg": [rx, ry, rz],
        "blender_version": bpy.app.version_string,
        "frames": [],
    }

    for clip_name, time_s in clips:
        apply_clip_time(armature, clip_name, time_s)
        frame_armature(armature, camera)
        safe = clip_name.replace(" ", "_")
        out_path = args.out_dir / f"{args.label}_{safe}_t{time_s:.2f}.png"
        bpy.context.scene.render.filepath = str(out_path)
        bpy.ops.render.render(write_still=True)
        manifest["frames"].append(
            {"clip": clip_name, "time_s": time_s, "png": str(out_path.name)}
        )

    (args.out_dir / f"{args.label}_manifest.json").write_text(
        json.dumps(manifest, indent=2), encoding="utf-8"
    )


if __name__ == "__main__":
    main()
