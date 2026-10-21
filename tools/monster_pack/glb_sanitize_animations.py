#!/usr/bin/env python3
"""
Strip child-bone translation and all scale from glTF animations.

Unity FBX clips often key translation on every bone; glTF export keeps those channels
even when Blender fcurves were removed. Cavecrawler's direct converter only animated
root translation plus bone rotations.
"""

from __future__ import annotations

import argparse
import json
import struct
from pathlib import Path

DEFAULT_TRANSLATION_BONES = frozenset({"ROOT_", "CAVECRAWLER_"})

# Blender io_scene_fbx armature object rotation (+90 deg X), as exported on working SK FBX GLBs.
SCENE_ROOT_ROTATION_X90 = [0.7071068286895752, 0.0, 0.0, 0.7071068286895752]


def load_glb(path: Path) -> tuple[dict, bytes]:
    data = path.read_bytes()
    if data[:4] != b"glTF":
        raise ValueError(f"Not a GLB: {path}")
    jlen = struct.unpack_from("<I", data, 12)[0]
    jstart = 20
    gltf = json.loads(data[jstart : jstart + jlen])
    bin_header = jstart + jlen
    if bin_header + 8 > len(data):
        raise ValueError(f"GLB missing BIN chunk: {path}")
    blob_len = struct.unpack_from("<I", data, bin_header)[0]
    chunk_type = data[bin_header + 4 : bin_header + 8]
    if chunk_type != b"BIN\x00":
        raise ValueError(f"GLB second chunk is not BIN: {path}")
    blob = data[bin_header + 8 : bin_header + 8 + blob_len]
    return gltf, blob


def write_glb(path: Path, gltf: dict, blob: bytes) -> None:
    json_bytes = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_pad = (4 - (len(json_bytes) % 4)) % 4
    json_bytes += b" " * json_pad
    blob_pad = (4 - (len(blob) % 4)) % 4
    blob_padded = blob + b"\x00" * blob_pad
    total = 12 + 8 + len(json_bytes) + 8 + len(blob_padded)
    header = struct.pack("<4sII", b"glTF", 2, total)
    chunk0 = struct.pack("<I4s", len(json_bytes), b"JSON") + json_bytes
    chunk1 = struct.pack("<I4s", len(blob_padded), b"BIN\x00") + blob_padded
    path.write_bytes(header + chunk0 + chunk1)


def _animation_input_accessor_indices(anim: dict) -> set[int]:
    out: set[int] = set()
    for ch in anim.get("channels", []):
        sampler = anim["samplers"][ch["sampler"]]
        out.add(int(sampler["input"]))
    return out


def _accessor_float_times(blob: bytes | bytearray, gltf: dict, accessor_index: int) -> tuple[int, int, int]:
    acc = gltf["accessors"][accessor_index]
    if acc.get("type") != "SCALAR" or acc.get("componentType") != 5126:
        raise ValueError(f"accessor {accessor_index}: expected FLOAT SCALAR time keys")
    bv = gltf["bufferViews"][acc["bufferView"]]
    byte_offset = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    count = int(acc["count"])
    return byte_offset, count, 4


def rebase_gltf_animation_starts_to_zero(gltf: dict, blob: bytes) -> tuple[bytes, int]:
    """
    Blender NLA_TRACKS export often leaves each glTF animation on the merged timeline
    (e.g. IdleBreathe keys at 9.87-11.87s). Shift each clip so its first key is t=0.
    """
    mutable = bytearray(blob)
    rebased_clips = 0
    for anim in gltf.get("animations", []):
        input_accessors = _animation_input_accessor_indices(anim)
        min_t: float | None = None
        for accessor_index in input_accessors:
            off, count, stride = _accessor_float_times(mutable, gltf, accessor_index)
            for i in range(count):
                t = struct.unpack_from("<f", mutable, off + i * stride)[0]
                min_t = t if min_t is None else min(min_t, t)
        if min_t is None or min_t < 1e-4:
            continue
        for accessor_index in input_accessors:
            off, count, stride = _accessor_float_times(mutable, gltf, accessor_index)
            for i in range(count):
                t = struct.unpack_from("<f", mutable, off + i * stride)[0]
                struct.pack_into("<f", mutable, off + i * stride, t - min_t)
        rebased_clips += 1
    return bytes(mutable), rebased_clips


def sanitize_gltf_animations(
    gltf: dict,
    translation_bones: frozenset[str],
) -> tuple[int, int]:
    nodes = gltf.get("nodes", [])
    removed = 0
    kept = 0
    for anim in gltf.get("animations", []):
        new_channels: list[dict] = []
        for ch in anim.get("channels", []):
            path = ch.get("target", {}).get("path", "")
            node_index = ch.get("target", {}).get("node")
            name = ""
            if node_index is not None and node_index < len(nodes):
                name = nodes[node_index].get("name", "")
            if path == "scale":
                removed += 1
                continue
            if path == "translation" and name not in translation_bones:
                removed += 1
                continue
            new_channels.append(ch)
            kept += 1
        anim["channels"] = new_channels
    return removed, kept


def apply_scene_root_rotation_x90(path: Path) -> None:
    """FBX2glTF -> Blender glTF export drops armature object rotation; patch scene root."""
    gltf, blob = load_glb(path)
    if not gltf.get("scenes"):
        raise ValueError(f"{path}: no scenes")
    scene = gltf["scenes"][0]
    if not scene.get("nodes"):
        raise ValueError(f"{path}: empty scene")
    root_index = scene["nodes"][0]
    gltf["nodes"][root_index]["rotation"] = list(SCENE_ROOT_ROTATION_X90)
    write_glb(path, gltf, blob)


def finish_glb_post_export(
    path: Path,
    translation_bones: frozenset[str],
    *,
    strip_child_translation_and_scale: bool = False,
    rebase_clip_starts: bool = True,
    in_place: bool = True,
) -> tuple[int, int, int]:
    """
    Default monster-pack post-export: rebase clip times only.
    Channel stripping is opt-in (see IMPORT_FIDELITY.md).
    """
    gltf, blob = load_glb(path)
    expected = gltf.get("buffers", [{}])[0].get("byteLength")
    if expected is not None and expected != len(blob):
        raise ValueError(
            f"{path}: BIN chunk is {len(blob)} bytes but buffers[0].byteLength is {expected}"
        )
    removed, kept = (0, 0)
    if strip_child_translation_and_scale:
        removed, kept = sanitize_gltf_animations(gltf, translation_bones)
    rebased = 0
    if rebase_clip_starts:
        blob, rebased = rebase_gltf_animation_starts_to_zero(gltf, blob)
    out = path if in_place else path.with_suffix(".postexport.glb")
    write_glb(out, gltf, blob)
    return removed, kept, rebased


def sanitize_file(
    path: Path,
    translation_bones: frozenset[str],
    in_place: bool = True,
    *,
    strip_child_translation_and_scale: bool = True,
) -> tuple[int, int]:
    removed, kept, _rebased = finish_glb_post_export(
        path,
        translation_bones,
        strip_child_translation_and_scale=strip_child_translation_and_scale,
        rebase_clip_starts=True,
        in_place=in_place,
    )
    return removed, kept


def rebase_animation_starts_file(path: Path) -> int:
    """Rebase clip times only (no channel stripping). Safe to run on an exported GLB."""
    gltf, blob = load_glb(path)
    blob, rebased = rebase_gltf_animation_starts_to_zero(gltf, blob)
    write_glb(path, gltf, blob)
    return rebased


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("glb", type=Path)
    parser.add_argument(
        "--translation-bone",
        action="append",
        default=[],
        help="Bone name allowed to keep translation channels (repeatable).",
    )
    parser.add_argument("--no-in-place", action="store_true")
    parser.add_argument(
        "--strip-child-channels",
        action="store_true",
        help="Remove child translation and all scale channels (legacy pipelines only).",
    )
    parser.add_argument(
        "--rebase-only",
        action="store_true",
        help="Shift animation keys to t=0 without stripping channels.",
    )
    args = parser.parse_args()
    bones = DEFAULT_TRANSLATION_BONES | frozenset(args.translation_bone or ["ROOT_"])
    path = args.glb.resolve()
    if args.rebase_only:
        rebased = rebase_animation_starts_file(path)
        print(f"{path}: rebased {rebased} clips")
        return
    strip = args.strip_child_channels
    if not strip:
        removed, kept, rebased = finish_glb_post_export(
            path, bones, strip_child_translation_and_scale=False, in_place=not args.no_in_place
        )
        print(f"{path}: rebased {rebased} clips (no channel strip)")
        return
    removed, kept = sanitize_file(path, bones, in_place=not args.no_in_place, strip_child_translation_and_scale=True)
    print(f"{args.glb}: removed {removed} channels, kept {kept}")


if __name__ == "__main__":
    main()
