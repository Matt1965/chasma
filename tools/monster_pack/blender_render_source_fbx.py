#!/usr/bin/env python3
"""Render a single animation FBX from the Unity pack (source reference)."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import bpy
from mathutils import Euler, Vector

import math


def parse_args() -> argparse.Namespace:
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1 :]
    else:
        argv = []
    parser = argparse.ArgumentParser()
    parser.add_argument("--fbx", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--time-s", type=float, default=0.0)
    parser.add_argument("--outer-rotation-deg", default="0,90,0")
    parser.add_argument("--resolution", type=int, default=512)
    return parser.parse_args(argv)


def clear_scene() -> None:
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete()
    for action in list(bpy.data.actions):
        bpy.data.actions.remove(action)


def main() -> None:
    args = parse_args()
    clear_scene()
    bpy.context.scene.render.engine = "BLENDER_EEVEE_NEXT"
    bpy.context.scene.render.resolution_x = args.resolution
    bpy.context.scene.render.resolution_y = args.resolution
    bpy.ops.import_scene.fbx(filepath=str(args.fbx), global_scale=1.0, use_anim=True)
    armature = next((o for o in bpy.data.objects if o.type == "ARMATURE"), None)
    if armature is None:
        raise SystemExit(f"No armature in {args.fbx}")
    rx, ry, rz = (float(v) for v in args.outer_rotation_deg.split(","))
    wrapper = bpy.data.objects.new("SourceWrapper", None)
    bpy.context.collection.objects.link(wrapper)
    armature.parent = wrapper
    wrapper.rotation_euler = Euler((math.radians(rx), math.radians(ry), math.radians(rz)), "XYZ")
    if armature.animation_data and armature.animation_data.action:
        frame = int(round(args.time_s * bpy.context.scene.render.fps))
        bpy.context.scene.frame_set(frame)
    bpy.ops.object.light_add(type="SUN", location=(4, -3, 6))
    bpy.ops.object.camera_add()
    camera = bpy.context.active_object
    bpy.context.scene.camera = camera
    bpy.context.view_layer.update()
    corners = [armature.matrix_world @ Vector(c) for c in armature.bound_box]
    min_v = Vector((min(c[i] for c in corners) for i in range(3)))
    max_v = Vector((max(c[i] for c in corners) for i in range(3)))
    center = (min_v + max_v) * 0.5
    size = max((max_v - min_v).length, 0.5)
    target = center + Vector((0.0, size * 0.15, size * 0.2))
    camera.location = target + Vector((size * 1.4, -size * 1.8, size * 0.55))
    camera.rotation_euler = (target - camera.location).to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.render.filepath = str(args.out)
    bpy.ops.render.render(write_still=True)


if __name__ == "__main__":
    main()
