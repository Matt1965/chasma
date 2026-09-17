#!/usr/bin/env python3
"""Author CG2 semantic morph targets on Chasma human unit GLBs.

Adds six technical targets shared by male/female variants:
  build_broad, build_narrow, fat_soft, muscle_define, head_large, head_small

Called from retarget_ual_to_human.py before save so morphs survive regeneration.
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

import numpy as np

MORPH_TARGET_NAMES: tuple[str, ...] = (
    "build_broad",
    "build_narrow",
    "fat_soft",
    "muscle_define",
    "head_large",
    "head_small",
)

HEAD_JOINT_SUFFIXES = ("head", "neck")
TORSO_JOINT_SUFFIXES = ("spine", "pelvis", "chest")
LIMB_JOINT_SUFFIXES = ("upperarm", "lowerarm", "thigh", "calf", "shoulder")


def load_glb(path: Path) -> tuple[dict, bytearray]:
    data = path.read_bytes()
    off = 12
    cl, _ = struct.unpack_from("<II", data, off)
    off += 8
    js = json.loads(data[off : off + cl])
    bin_blob = bytearray()
    off = 12
    while off < len(data):
        cl, ty = struct.unpack_from("<II", data, off)
        off += 8
        chunk = data[off : off + cl]
        off += cl
        if ty == 0x004E4942:
            bin_blob = bytearray(chunk)
    return js, bin_blob


def read_accessor(js: dict, blob: bytearray, accessor_idx: int) -> np.ndarray:
    acc = js["accessors"][accessor_idx]
    bv = js["bufferViews"][acc["bufferView"]]
    start = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    count = acc["count"]
    ctype = acc["componentType"]
    atype = acc["type"]
    comps = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[atype]
    dtype = {
        5126: np.float32,
        5121: np.uint8,
        5123: np.uint16,
        5125: np.uint32,
    }[ctype]
    stride = bv.get("byteStride", comps * np.dtype(dtype).itemsize)
    raw = blob[start : start + count * stride]
    arr = np.frombuffer(raw, dtype=dtype, count=count * comps)
    return arr.reshape(count, comps)


def append_bytes(blob: bytearray, data: bytes) -> int:
    off = len(blob)
    blob.extend(data)
    return off


def write_accessor(js: dict, blob: bytearray, values: np.ndarray, atype: str) -> int:
    flat = np.asarray(values, dtype=np.float32).reshape(-1)
    raw = flat.tobytes()
    byte_offset = append_bytes(blob, raw)
    bv_idx = len(js["bufferViews"])
    js["bufferViews"].append(
        {
            "buffer": 0,
            "byteOffset": byte_offset,
            "byteLength": len(raw),
        }
    )
    comps = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[atype]
    acc_idx = len(js["accessors"])
    js["accessors"].append(
        {
            "bufferView": bv_idx,
            "componentType": 5126,
            "count": len(flat) // comps,
            "type": atype,
        }
    )
    return acc_idx


def joint_indices_for_suffixes(joint_names: list[str], suffixes: tuple[str, ...]) -> set[int]:
    allowed: set[int] = set()
    for ji, name in enumerate(joint_names):
        lower = name.lower()
        if any(lower.endswith(suffix) or suffix in lower for suffix in suffixes):
            allowed.add(ji)
    return allowed


def skin_joint_names(js: dict, skin_idx: int) -> list[str]:
    skin = js["skins"][skin_idx]
    nodes = js["nodes"]
    return [nodes[j].get("name", f"joint_{j}") for j in skin["joints"]]


def compute_morph_deltas(
    positions: np.ndarray,
    normals: np.ndarray,
    joints: np.ndarray,
    weights: np.ndarray,
    joint_names: list[str],
    mesh_name: str,
) -> dict[str, np.ndarray]:
    count = positions.shape[0]
    zeros = np.zeros((count, 3), dtype=np.float32)
    deltas = {name: zeros.copy() for name in MORPH_TARGET_NAMES}

    head_mask = np.zeros(count, dtype=np.float32)
    torso_mask = np.zeros(count, dtype=np.float32)
    limb_mask = np.zeros(count, dtype=np.float32)

    head_joints = joint_indices_for_suffixes(joint_names, HEAD_JOINT_SUFFIXES)
    torso_joints = joint_indices_for_suffixes(joint_names, TORSO_JOINT_SUFFIXES)
    limb_joints = joint_indices_for_suffixes(joint_names, LIMB_JOINT_SUFFIXES)

    for vi in range(count):
        for ji, w in zip(joints[vi], weights[vi], strict=False):
            if w <= 0.0:
                continue
            if ji in head_joints:
                head_mask[vi] = max(head_mask[vi], w)
            if ji in torso_joints:
                torso_mask[vi] = max(torso_mask[vi], w)
            if ji in limb_joints:
                limb_mask[vi] = max(limb_mask[vi], w)

    mesh_lower = mesh_name.lower()
    is_face_mesh = "eye" in mesh_lower or "brow" in mesh_lower or "face" in mesh_lower

    center_x = np.median(positions[:, 0])
    rel_x = positions[:, 0] - center_x
    side = np.sign(rel_x)
    side[side == 0] = 1.0

    if not is_face_mesh:
        broad = torso_mask[:, None] * np.stack(
            [side * 0.035, np.zeros(count), np.zeros(count)], axis=1
        )
        narrow = torso_mask[:, None] * np.stack(
            [-side * 0.03, np.zeros(count), np.zeros(count)], axis=1
        )
        fat = (torso_mask + limb_mask * 0.5)[:, None] * normals * 0.028
        muscle = limb_mask[:, None] * normals * 0.022
        deltas["build_broad"] = broad.astype(np.float32)
        deltas["build_narrow"] = narrow.astype(np.float32)
        deltas["fat_soft"] = fat.astype(np.float32)
        deltas["muscle_define"] = muscle.astype(np.float32)

    head_center = positions[head_mask > 0.1].mean(axis=0) if np.any(head_mask > 0.1) else positions.mean(axis=0)
    head_vec = positions - head_center
    head_large = head_mask[:, None] * head_vec * 0.12
    head_small = head_mask[:, None] * head_vec * -0.10
    deltas["head_large"] = head_large.astype(np.float32)
    deltas["head_small"] = head_small.astype(np.float32)

    return deltas


def set_bevy_mesh_morph_target_names(mesh: dict) -> None:
    """Bevy 0.18 glTF loader reads names from mesh extras, not per-target `name` fields."""
    # Must be a JSON object in the glTF root — not a stringified JSON blob.
    mesh["extras"] = {"targetNames": list(MORPH_TARGET_NAMES)}


def add_morph_targets_to_primitive(
    js: dict,
    blob: bytearray,
    primitive: dict,
    mesh_name: str,
    skin_idx: int,
) -> None:
    attrs = primitive["attributes"]
    if "JOINTS_0" not in attrs or "POSITION" not in attrs:
        return
    positions = read_accessor(js, blob, attrs["POSITION"])
    normals = read_accessor(js, blob, attrs["NORMAL"])
    joints = read_accessor(js, blob, attrs["JOINTS_0"])
    weights = read_accessor(js, blob, attrs["WEIGHTS_0"])
    if weights.shape[1] > 4:
        weights = weights[:, :4]
    if joints.shape[1] > 4:
        joints = joints[:, :4]

    joint_names = skin_joint_names(js, skin_idx)
    deltas = compute_morph_deltas(positions, normals, joints, weights, joint_names, mesh_name)

    targets = []
    for target_name in MORPH_TARGET_NAMES:
        acc = write_accessor(js, blob, deltas[target_name], "VEC3")
        targets.append({"POSITION": acc, "name": target_name})
    primitive["targets"] = targets


def author_morph_targets(js: dict, blob: bytearray) -> None:
    """Mutate glTF JSON + BIN in place, adding morph targets to skinned meshes."""
    meshes_with_morphs: set[int] = set()
    for mesh_idx, mesh in enumerate(js.get("meshes", [])):
        mesh_name = mesh.get("name", f"mesh_{mesh_idx}")
        for primitive in mesh.get("primitives", []):
            if "JOINTS_0" not in primitive.get("attributes", {}):
                continue
            skin_idx = 0
            for node in js.get("nodes", []):
                if node.get("mesh") == mesh_idx and node.get("skin") is not None:
                    skin_idx = node["skin"]
                    break
            add_morph_targets_to_primitive(js, blob, primitive, mesh_name, skin_idx)
            meshes_with_morphs.add(mesh_idx)

    for mesh_idx in meshes_with_morphs:
        set_bevy_mesh_morph_target_names(js["meshes"][mesh_idx])

    if js.get("bufferViews"):
        js["buffers"] = [{"byteLength": len(blob)}]


def author_glb_file(path: Path) -> None:
    js, blob = load_glb(path)
    author_morph_targets(js, blob)
    json_bytes = json.dumps(js, separators=(",", ":")).encode("utf-8")
    json_pad = (4 - len(json_bytes) % 4) % 4
    json_bytes += b" " * json_pad
    bin_pad = (4 - len(blob) % 4) % 4
    blob.extend(b"\x00" * bin_pad)
    total = 12 + 8 + len(json_bytes) + 8 + len(blob)
    out = bytearray()
    out.extend(struct.pack("<III", 0x46546C67, 2, total))
    out.extend(struct.pack("<II", len(json_bytes), 0x4E4F534A))
    out.extend(json_bytes)
    out.extend(struct.pack("<II", len(blob), 0x004E4942))
    out.extend(blob)
    path.write_bytes(out)
    print(f"authored morph targets on {path}")


if __name__ == "__main__":
    import sys

    repo = Path(__file__).resolve().parents[1]
    targets = sys.argv[1:] or [
        str(repo / "assets" / "units" / "human_male.glb"),
        str(repo / "assets" / "units" / "human_female.glb"),
    ]
    for target in targets:
        author_glb_file(Path(target))
