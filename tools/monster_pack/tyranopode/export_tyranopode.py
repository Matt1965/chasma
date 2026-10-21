#!/usr/bin/env python3
"""
Tyranopode-only exporter (reference monster).

Pipeline matches Cavecrawler: SK import -> direct action merge -> textures -> GLB.
No pose baking, no presentation-root scale, no pre-merge bind-pose edits.
"""

from __future__ import annotations

import sys
from pathlib import Path

TYRANOPODE_DIR = Path(
    r"C:\Users\matt1\My project\Assets\Monsters Full Pack Vol 1\Monsters Pack Vol 1\Tyranopode"
)
SK_FBX = TYRANOPODE_DIR / "FBX Files" / "SK_Tyranopode.FBX"
ANIM_PREFIX = "Tyranopode"
REPO = Path(__file__).resolve().parents[3]
OUTPUT = REPO / "assets" / "units" / "tyranopode.glb"

PACK_DIR = Path(__file__).resolve().parents[1]
if str(PACK_DIR) not in sys.path:
    sys.path.insert(0, str(PACK_DIR))

import bpy  # noqa: E402

from unity_materials import apply_textures_to_meshes  # noqa: E402

# Tyranopode SK FBX already carries a 0.01 object scale; global_scale=0.01 -> 1e-4 armature (breaks skinning).
FBX_SCALE = 1.0
FPS = 30


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


def find_armature() -> bpy.types.Object:
    for obj in bpy.data.objects:
        if obj.type == "ARMATURE":
            return obj
    raise RuntimeError("No armature")


def clip_name(fbx_name: str) -> str:
    stem = Path(fbx_name).stem
    prefix = f"{ANIM_PREFIX}@"
    return stem[len(prefix) :] if stem.startswith(prefix) else stem.split("@", 1)[-1]


def merge_action(armature: bpy.types.Object, action: bpy.types.Action, name: str) -> None:
    action.name = name
    if armature.animation_data is None:
        armature.animation_data_create()
    armature.animation_data.action = None
    track = armature.animation_data.nla_tracks.new()
    track.name = name
    strip = track.strips.new(name, 0, action)
    strip.action_frame_start = action.frame_range[0]
    strip.action_frame_end = action.frame_range[1]


def apply_object_scale(armature: bpy.types.Object) -> None:
    meshes = collect_meshes(armature)
    if max(armature.scale) > 0.999 and min(armature.scale) < 1.001:
        return
    bpy.ops.object.select_all(action="DESELECT")
    armature.select_set(True)
    for mesh in meshes:
        mesh.select_set(True)
    bpy.context.view_layer.objects.active = armature
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)


def delete_imported(objects: list[bpy.types.Object]) -> None:
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objects:
        if obj and obj.name in bpy.data.objects:
            obj.select_set(True)
    bpy.ops.object.delete()


def collect_meshes(armature: bpy.types.Object) -> list[bpy.types.Object]:
    out: list[bpy.types.Object] = []
    stack = list(armature.children)
    while stack:
        obj = stack.pop()
        if obj.type == "MESH":
            out.append(obj)
        stack.extend(obj.children)
    return out


def limit_skin_weights(max_influences: int = 4) -> None:
    for obj in bpy.data.objects:
        if obj.type != "MESH":
            continue
        for vert in obj.data.vertices:
            groups = list(vert.groups)
            if len(groups) <= max_influences:
                continue
            groups.sort(key=lambda g: g.weight, reverse=True)
            keep = groups[:max_influences]
            total = sum(g.weight for g in keep) or 1.0
            for g in vert.groups:
                g.weight = 0.0
            for g in keep:
                g.weight /= total


def strip_take001(armature: bpy.types.Object) -> None:
    if not armature.animation_data:
        return
    for track in list(armature.animation_data.nla_tracks):
        if "|" in track.name or "Take 001" in track.name:
            armature.animation_data.nla_tracks.remove(track)
    armature.animation_data.action = None
    for action in list(bpy.data.actions):
        if "|" in action.name or "Take 001" in action.name:
            bpy.data.actions.remove(action)


def export_glb(path: Path, armature: bpy.types.Object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.object.select_all(action="DESELECT")
    armature.select_set(True)
    bpy.context.view_layer.objects.active = armature
    for mesh in collect_meshes(armature):
        mesh.select_set(True)
    bpy.ops.export_scene.gltf(
        filepath=str(path),
        export_format="GLB",
        use_selection=True,
        export_apply=False,
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
    if not SK_FBX.is_file():
        raise SystemExit(f"Missing source SK: {SK_FBX}")
    fbx_dir = SK_FBX.parent

    clear_scene()
    import_fbx(SK_FBX)
    armature = find_armature()
    apply_object_scale(armature)
    strip_take001(armature)
    for action in list(bpy.data.actions):
        bpy.data.actions.remove(action)

    anim_paths = sorted(fbx_dir.glob(f"{ANIM_PREFIX}@*.FBX"))
    anim_paths += sorted(fbx_dir.glob(f"{ANIM_PREFIX}@*.fbx"))
    merged = 0
    for anim_path in anim_paths:
        if anim_path == SK_FBX:
            continue
        name = clip_name(anim_path.name)
        imported = import_fbx(anim_path)
        action = None
        source_armature = None
        for obj in imported:
            if obj.type == "ARMATURE":
                source_armature = obj
                if obj.animation_data and obj.animation_data.action:
                    action = obj.animation_data.action
        if source_armature is not None:
            apply_object_scale(source_armature)
        if action is None:
            delete_imported(imported)
            continue
        merge_action(armature, action, name)
        delete_imported(imported)
        merged += 1

    apply_textures_to_meshes(TYRANOPODE_DIR, "M_Tyranopode")
    limit_skin_weights(4)
    bpy.context.scene.render.fps = FPS
    export_glb(OUTPUT, armature)
    print(f"Merged {merged} clips -> {OUTPUT}")


if __name__ == "__main__":
    main()
