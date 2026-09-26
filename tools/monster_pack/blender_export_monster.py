"""
Blender batch export: Unity monster FBX pack -> single skinned GLB.

Run headless:
  blender --background --python blender_export_monster.py -- \\
    --source-dir "<monster folder>" \\
    --monster-name Gorosaurus \\
    --output "C:/path/gorosaurus.glb"
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import bpy

# Unity FBX centimeters -> glTF meters (matches Cavecrawler conversion notes).
FBX_SCALE = 0.01
FPS = 30


def parse_args() -> argparse.Namespace:
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1 :]
    else:
        argv = []
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-dir", type=Path, required=True)
    parser.add_argument("--monster-name", required=True)
    parser.add_argument("--anim-prefix", default="")
    parser.add_argument("--sk-fbx", type=Path, default=None)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--include-rm-clips", action="store_true", default=True)
    return parser.parse_args(argv)


def clear_scene() -> None:
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete()
    for action in list(bpy.data.actions):
        bpy.data.actions.remove(action)


def import_fbx(path: Path) -> list[bpy.types.Object]:
    bpy.ops.import_scene.fbx(
        filepath=str(path),
        global_scale=FBX_SCALE,
        automatic_bone_orientation=False,
        use_anim=True,
        ignore_leaf_bones=True,
        force_connect_children=False,
    )
    return list(bpy.context.selected_objects)


def find_armature(objects: list[bpy.types.Object]) -> bpy.types.Object | None:
    for obj in objects:
        if obj.type == "ARMATURE":
            return obj
    for obj in bpy.data.objects:
        if obj.type == "ARMATURE":
            return obj
    return None


def clip_name_from_fbx(monster: str, fbx_name: str) -> str:
    base = Path(fbx_name).stem
    prefix = f"{monster}@"
    if base.startswith(prefix):
        return base[len(prefix) :]
    if "@" in base:
        return base.split("@", 1)[1]
    return base


def merge_action_onto_armature(armature: bpy.types.Object, action: bpy.types.Action, name: str) -> None:
    action.name = name
    if armature.animation_data is None:
        armature.animation_data_create()
    if armature.animation_data.action and armature.animation_data.action.name != name:
        armature.animation_data.action = None
    track = armature.animation_data.nla_tracks.new()
    track.name = name
    strip = track.strips.new(name, 0, action)
    strip.action_frame_start = action.frame_range[0]
    strip.action_frame_end = action.frame_range[1]


def delete_objects(objects: list[bpy.types.Object]) -> None:
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objects:
        if obj and obj.name in bpy.data.objects:
            obj.select_set(True)
    bpy.ops.object.delete()


def downscale_textures(max_dim: int = 2048) -> None:
    """Keep GLBs under GitHub size limits; Chasma does not use Draco mesh compression."""
    for image in bpy.data.images:
        if image.size[0] == 0 or image.size[1] == 0:
            continue
        w, h = image.size[0], image.size[1]
        if w <= max_dim and h <= max_dim:
            continue
        scale = max_dim / float(max(w, h))
        image.scale(int(w * scale), int(h * scale))


def limit_skin_weights(max_influences: int = 4) -> None:
    for obj in bpy.data.objects:
        if obj.type != "MESH" or not obj.data.vertices:
            continue
        mesh = obj.data
        for vert in mesh.vertices:
            weights = list(vert.groups)
            if len(weights) <= max_influences:
                continue
            weights.sort(key=lambda g: g.weight, reverse=True)
            keep = weights[:max_influences]
            total = sum(g.weight for g in keep) or 1.0
            for g in vert.groups:
                g.weight = 0.0
            for g in keep:
                g.weight = g.weight / total


def export_glb(output: Path, armature: bpy.types.Object) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.object.select_all(action="DESELECT")
    armature.select_set(True)
    bpy.context.view_layer.objects.active = armature
    for child in armature.children:
        if child.type == "MESH":
            child.select_set(True)
    bpy.ops.export_scene.gltf(
        filepath=str(output),
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_animations=True,
        export_skins=True,
        export_all_influences=False,
        export_def_bones=False,
        export_materials="EXPORT",
        export_texcoords=True,
        export_normals=True,
        export_tangents=True,
        export_image_format="AUTO",
        export_yup=True,
    )


def main() -> None:
    args = parse_args()
    fbx_dir = args.source_dir / "FBX Files"
    if not fbx_dir.is_dir():
        raise SystemExit(f"Missing FBX Files: {fbx_dir}")

    if args.sk_fbx:
        sk_path = args.sk_fbx
    else:
        sk_candidates = sorted(fbx_dir.glob(f"SK_{args.monster_name}.FBX"))
        sk_candidates += sorted(fbx_dir.glob(f"SK_{args.monster_name}.fbx"))
        if not sk_candidates:
            sk_candidates = sorted(fbx_dir.glob("SK_*.FBX"))
        if not sk_candidates:
            raise SystemExit(f"No SK_ mesh for {args.monster_name}")
        sk_path = sk_candidates[0]

    anim_prefix = args.anim_prefix or args.monster_name

    clear_scene()
    base_objs = import_fbx(sk_path)
    armature = find_armature(base_objs)
    if armature is None:
        raise SystemExit("No armature in SK FBX")

    if armature.animation_data and armature.animation_data.nla_tracks:
        for track in list(armature.animation_data.nla_tracks):
            if "|" in track.name or "Take 001" in track.name:
                armature.animation_data.nla_tracks.remove(track)

    anim_paths = sorted(p for p in fbx_dir.glob(f"{anim_prefix}@*.FBX") if p != sk_path)
    anim_paths += sorted(p for p in fbx_dir.glob(f"{anim_prefix}@*.fbx") if p != sk_path)

    for anim_path in anim_paths:
        clip = clip_name_from_fbx(anim_prefix, anim_path.name)
        if not args.include_rm_clips and clip.endswith("_RM"):
            continue
        imported = import_fbx(anim_path)
        action = None
        for obj in imported:
            if obj.type == "ARMATURE" and obj.animation_data and obj.animation_data.action:
                action = obj.animation_data.action
                break
        if action is None:
            for action_candidate in bpy.data.actions:
                if action_candidate.users == 1:
                    action = action_candidate
                    break
        if action is None:
            delete_objects(imported)
            continue
        merge_action_onto_armature(armature, action, clip)
        delete_objects(imported)

    limit_skin_weights(4)
    downscale_textures(1024)
    bpy.context.scene.render.fps = FPS
    export_glb(args.output, armature)
    print(f"Exported {args.output}")


if __name__ == "__main__":
    main()
