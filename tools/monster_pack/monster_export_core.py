"""Unity monster FBX -> Chasma GLB (faithful import; see IMPORT_FIDELITY.md)."""

from __future__ import annotations

import math
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

import bpy

from glb_fbx2gltf_scene_fix import apply_fbx2gltf_scene_root_x90
from glb_sanitize_animations import finish_glb_post_export
from unity_materials import apply_textures_to_meshes

FBX_SCALE = 1.0
FPS = 30

FBX2GLTF_CANDIDATES = (
    Path(__file__).resolve().parent / "bin" / "FBX2glTF-windows-x86_64.exe",
    Path(r"C:\BevyFiles\Temp\FBX2glTF-win\FBX2glTF-windows-x86_64\FBX2glTF-windows-x86_64.exe"),
)


def resolve_fbx2gltf_exe() -> Path | None:
    env = os.environ.get("MONSTER_FBX2GLTF_EXE")
    if env and Path(env).is_file():
        return Path(env)
    for path in FBX2GLTF_CANDIDATES:
        if path.is_file():
            return path
    return None


def run_fbx2gltf(sk_fbx: Path, output_glb: Path) -> None:
    exe = resolve_fbx2gltf_exe()
    if exe is None:
        raise RuntimeError(
            "FBX2glTF not found. Run tools/monster_pack/ensure_fbx2gltf.ps1 "
            "or set MONSTER_FBX2GLTF_EXE."
        )
    output_glb.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [
            str(exe),
            "--input",
            str(sk_fbx),
            "--output",
            str(output_glb.with_suffix("")),
            "--binary",
        ],
        check=True,
    )
    if not output_glb.is_file():
        raise RuntimeError(f"FBX2glTF did not write {output_glb}")


def apply_blender_fbx_armature_orientation(armature: bpy.types.Object) -> None:
    """Match io_scene_fbx: skeletal root gets +90 deg X on the armature object."""
    armature.rotation_euler = (math.pi / 2.0, 0.0, 0.0)


def delete_loose_meshes(armature: bpy.types.Object) -> None:
    keep = set(collect_meshes(armature))
    for obj in list(bpy.data.objects):
        if obj.type == "MESH" and obj not in keep:
            bpy.data.objects.remove(obj, do_unlink=True)


def import_sk_armature(sk_fbx: Path, scratch_dir: Path) -> tuple[bpy.types.Object, bool]:
    """Import SK mesh + rig; Blender FBX first, then Godot FBX2glTF -> glTF."""
    clear_scene()
    try:
        import_fbx(sk_fbx)
        armature = find_armature()
        if armature is None:
            raise RuntimeError("no armature after FBX import")
        if not collect_meshes(armature):
            raise RuntimeError("no skinned mesh after FBX import")
        apply_object_scale(armature)
        delete_loose_meshes(armature)
        return armature, False
    except RuntimeError:
        clear_scene()

    glb_path = scratch_dir / f"{sk_fbx.stem}_fbx2gltf.glb"
    run_fbx2gltf(sk_fbx, glb_path)
    bpy.ops.import_scene.gltf(filepath=str(glb_path))
    armature = find_armature()
    if armature is None:
        raise RuntimeError(f"FBX2glTF import produced no armature: {sk_fbx}")
    delete_loose_meshes(armature)
    apply_object_scale(armature)
    apply_blender_fbx_armature_orientation(armature)
    return armature, True


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


def find_armature(objects: list[bpy.types.Object] | None = None) -> bpy.types.Object | None:
    if objects:
        for obj in objects:
            if obj.type == "ARMATURE":
                return obj
    for obj in bpy.data.objects:
        if obj.type == "ARMATURE":
            return obj
    return None


def clip_name_from_fbx(anim_prefix: str, fbx_name: str) -> str:
    stem = Path(fbx_name).stem
    prefix = f"{anim_prefix}@"
    if stem.startswith(prefix):
        return stem[len(prefix) :]
    if "@" in stem:
        return stem.split("@", 1)[1]
    return stem


def infer_translation_bones(armature: bpy.types.Object) -> frozenset[str]:
    """Bones that may keep translation channels when legacy channel strip is enabled."""
    names: set[str] = {"ROOT_", "root", "Root"}
    for bone in armature.data.bones:
        if bone.parent is None:
            names.add(bone.name)
    return frozenset(names)


def sanitise_action_channels(
    action: bpy.types.Action, translation_bones: frozenset[str]
) -> None:
    for fcurve in list(action.fcurves):
        path = fcurve.data_path
        if not path.startswith("pose.bones") or '"' not in path:
            continue
        bone = path.split('"', 2)[1]
        if "scale" in path:
            action.fcurves.remove(fcurve)
            continue
        if "location" in path and bone not in translation_bones:
            action.fcurves.remove(fcurve)


def merge_action(
    action: bpy.types.Action,
    name: str,
    translation_bones: frozenset[str],
    *,
    strip_fcurves: bool = False,
) -> None:
    if strip_fcurves:
        sanitise_action_channels(action, translation_bones)
    action.name = name
    action.use_fake_user = True


def collect_meshes(armature: bpy.types.Object) -> list[bpy.types.Object]:
    out: list[bpy.types.Object] = []
    stack = list(armature.children)
    while stack:
        obj = stack.pop()
        if obj.type == "MESH":
            out.append(obj)
        stack.extend(obj.children)
    return out


def apply_object_scale(armature: bpy.types.Object) -> None:
    if max(armature.scale) > 0.999 and min(armature.scale) < 1.001:
        return
    meshes = collect_meshes(armature)
    bpy.ops.object.select_all(action="DESELECT")
    armature.select_set(True)
    for mesh in meshes:
        mesh.select_set(True)
    bpy.context.view_layer.objects.active = armature
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)


def delete_objects(objects: list[bpy.types.Object]) -> None:
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objects:
        if obj and obj.name in bpy.data.objects:
            obj.select_set(True)
    bpy.ops.object.delete()


def is_junk_action_name(name: str) -> bool:
    return "|" in name or "Take 001" in name or "BaseLayer" in name


def strip_take001(armature: bpy.types.Object) -> None:
    if not armature.animation_data:
        return
    for track in list(armature.animation_data.nla_tracks):
        if is_junk_action_name(track.name):
            armature.animation_data.nla_tracks.remove(track)
    armature.animation_data.action = None
    purge_junk_actions()


def purge_junk_actions(allowed: set[str] | None = None) -> None:
    for action in list(bpy.data.actions):
        if is_junk_action_name(action.name):
            bpy.data.actions.remove(action)
            continue
        if allowed is not None and action.name not in allowed:
            bpy.data.actions.remove(action)


def downscale_textures(max_dim: int = 2048) -> None:
    for image in bpy.data.images:
        if image.size[0] == 0 or image.size[1] == 0:
            continue
        w, h = image.size[0], image.size[1]
        if w <= max_dim and h <= max_dim:
            continue
        scale = max_dim / float(max(w, h))
        image.scale(int(w * scale), int(h * scale))


def push_merged_actions_to_nla(armature: bpy.types.Object) -> None:
    """Godot FBX2glTF rigs often fail glTF ACTIONS export; NLA strips work."""
    if armature.animation_data is None:
        armature.animation_data_create()
    armature.animation_data.action = None
    for track in list(armature.animation_data.nla_tracks):
        armature.animation_data.nla_tracks.remove(track)
    cursor = 0
    for action in sorted(bpy.data.actions, key=lambda item: item.name):
        if not action.use_fake_user or is_junk_action_name(action.name):
            continue
        start = int(action.frame_range[0])
        end = int(action.frame_range[1])
        track = armature.animation_data.nla_tracks.new()
        track.name = action.name
        track.strips.new(action.name, cursor, action)
        cursor += max(end - start, 1) + 2


def export_glb(path: Path, armature: bpy.types.Object, *, use_nla_tracks: bool) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    animation_mode = "NLA_TRACKS" if use_nla_tracks else "ACTIONS"
    if use_nla_tracks:
        push_merged_actions_to_nla(armature)
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
        export_animation_mode=animation_mode,
        export_bake_animation=False,
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


def export_monster_glb(
    *,
    source_dir: Path,
    monster_name: str,
    output: Path,
    anim_prefix: str,
    sk_fbx: Path,
    include_rm_clips: bool = True,
    texture_max_dim: int = 2048,
    checkpoint_dir: Path | None = None,
    strip_child_channels: bool = False,
    strip_blender_fcurves: bool = False,
) -> tuple[int, int, int, int]:
    """Returns (merged_clip_count, stripped_channels, kept_channels, rebased_clips)."""
    fbx_dir = source_dir / "FBX Files"
    if not sk_fbx.is_file():
        raise FileNotFoundError(f"Missing SK FBX: {sk_fbx}")

    scratch_dir = Path(tempfile.mkdtemp(prefix="chasma_monster_export_"))
    armature, used_fbx2gltf_sk = import_sk_armature(sk_fbx, scratch_dir)
    strip_take001(armature)
    translation_bones = infer_translation_bones(armature)
    for action in list(bpy.data.actions):
        bpy.data.actions.remove(action)

    anim_paths = sorted(p for p in fbx_dir.glob(f"{anim_prefix}@*.FBX") if p != sk_fbx)
    anim_paths += sorted(p for p in fbx_dir.glob(f"{anim_prefix}@*.fbx") if p != sk_fbx)

    merged = 0
    seen_names: set[str] = set()
    for anim_path in anim_paths:
        clip = clip_name_from_fbx(anim_prefix, anim_path.name)
        if clip in seen_names:
            continue
        if not include_rm_clips and clip.endswith("_RM"):
            continue
        imported = import_fbx(anim_path)
        source_armature = find_armature(imported)
        action = None
        if source_armature and source_armature.animation_data:
            action = source_armature.animation_data.action
        if source_armature is not None:
            apply_object_scale(source_armature)
        if action is None:
            delete_objects(imported)
            continue
        merge_action(
            action,
            clip,
            translation_bones,
            strip_fcurves=strip_blender_fcurves,
        )
        delete_objects(imported)
        purge_junk_actions()
        seen_names.add(clip)
        merged += 1

    purge_junk_actions(seen_names)
    mat_name = f"M_{monster_name.replace(' ', '')}"
    apply_textures_to_meshes(source_dir, mat_name)
    downscale_textures(texture_max_dim)
    bpy.context.scene.render.fps = FPS

    if checkpoint_dir is not None:
        checkpoint_dir.mkdir(parents=True, exist_ok=True)
        blender_export = checkpoint_dir / "01_blender_gltf_export.glb"
        before_post = checkpoint_dir / "02_before_glb_postprocess.glb"
        faithful = checkpoint_dir / "03_faithful_postexport.glb"
    else:
        blender_export = output
        before_post = None
        faithful = output

    export_glb(blender_export, armature, use_nla_tracks=used_fbx2gltf_sk)
    if before_post is not None:
        shutil.copy2(blender_export, before_post)
        shutil.copy2(blender_export, faithful)

    work_path = faithful if checkpoint_dir is not None else output
    removed, kept, rebased = finish_glb_post_export(
        work_path,
        translation_bones,
        strip_child_translation_and_scale=strip_child_channels,
        rebase_clip_starts=True,
    )
    if used_fbx2gltf_sk:
        apply_fbx2gltf_scene_root_x90(work_path)

    if checkpoint_dir is not None:
        shutil.copy2(work_path, output)

    return merged, removed, kept, rebased
