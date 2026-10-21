#!/usr/bin/env python3
"""Sample armature bone pose from a pack animation FBX (diagnostic)."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import bpy
from mathutils import Vector


def parse_args() -> argparse.Namespace:
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1 :]
    else:
        argv = []
    p = argparse.ArgumentParser()
    p.add_argument("--fbx", type=Path, required=True)
    p.add_argument("--time-s", type=float, default=0.0)
    p.add_argument("--bones", default="root,Bufomorph_,Bufomorph_JawUpper")
    return p.parse_args(argv)


def clear_scene() -> None:
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete()
    for action in list(bpy.data.actions):
        bpy.data.actions.remove(action)


def main() -> None:
    args = parse_args()
    clear_scene()
    bpy.ops.import_scene.fbx(filepath=str(args.fbx), global_scale=1.0, use_anim=True)
    armature = next((o for o in bpy.data.objects if o.type == "ARMATURE"), None)
    if armature is None:
        raise SystemExit("no armature")
    frame = int(round(args.time_s * bpy.context.scene.render.fps))
    bpy.context.scene.frame_set(frame)
    bpy.context.view_layer.update()
    bones = [b.strip() for b in args.bones.split(",") if b.strip()]
    out = {"fbx": str(args.fbx), "time_s": args.time_s, "frame": frame, "bones": {}}
    for name in bones:
        if name not in armature.pose.bones:
            out["bones"][name] = {"missing": True}
            continue
        pb = armature.pose.bones[name]
        mat = armature.matrix_world @ pb.matrix
        loc = mat.to_translation()
        rot = mat.to_quaternion()
        out["bones"][name] = {
            "world_loc": [loc.x, loc.y, loc.z],
            "world_quat": [rot.x, rot.y, rot.z, rot.w],
            "local_loc": list(pb.location),
        }
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
