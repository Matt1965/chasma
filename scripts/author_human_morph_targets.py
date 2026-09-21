#!/usr/bin/env python3
"""Author semantic morph targets on Chasma human unit GLBs.

CG2 coarse targets plus CG9 regional body shaping. Called from
retarget_ual_to_human.py before save so morphs survive regeneration.
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

import numpy as np

from glb_geometry import triangle_indices, vertex_neighbors

CG2_MORPH_TARGET_NAMES: tuple[str, ...] = (
    "build_broad",
    "build_narrow",
    "fat_soft",
    "muscle_define",
    "head_large",
    "head_small",
)

REGIONAL_MORPH_TARGET_NAMES: tuple[str, ...] = (
    "shoulders_broad",
    "shoulders_narrow",
    "torso_broad",
    "torso_narrow",
    "arms_thick",
    "arms_thin",
    "hips_broad",
    "hips_narrow",
    "legs_thick",
    "legs_thin",
)

MORPH_TARGET_NAMES: tuple[str, ...] = CG2_MORPH_TARGET_NAMES + REGIONAL_MORPH_TARGET_NAMES

SEAM_VERTEX_EPS = 0.0015
REGIONAL_MAGNITUDE_SCALE = 0.90
DELTA_SMOOTH_ITERS = 2
DELTA_SMOOTH_ALPHA = 0.30

HEAD_JOINT_SUFFIXES = ("head", "neck")
TORSO_JOINT_SUFFIXES = ("spine", "pelvis", "chest")
LIMB_JOINT_SUFFIXES = ("upperarm", "lowerarm", "thigh", "calf", "shoulder")

SHOULDER_JOINT_SUFFIXES = ("clavicle",)
ARM_UPPER_JOINT_SUFFIXES = ("upperarm",)
ARM_LOWER_JOINT_SUFFIXES = ("lowerarm",)
HAND_JOINT_SUFFIXES = ("hand", "thumb", "index", "middle", "ring", "pinky")
SPINE_JOINT_SUFFIXES = ("spine_01", "spine_02", "spine_03")
PELVIS_JOINT_SUFFIXES = ("pelvis",)
THIGH_JOINT_SUFFIXES = ("thigh",)
CALF_JOINT_SUFFIXES = ("calf",)
FOOT_JOINT_SUFFIXES = ("foot", "ball")


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


def vertex_influence_mask(
    count: int,
    joints: np.ndarray,
    weights: np.ndarray,
    joint_names: list[str],
    suffixes: tuple[str, ...],
) -> np.ndarray:
    allowed = joint_indices_for_suffixes(joint_names, suffixes)
    mask = np.zeros(count, dtype=np.float32)
    for vi in range(count):
        for ji, w in zip(joints[vi], weights[vi], strict=False):
            if w <= 0.0 or ji not in allowed:
                continue
            mask[vi] = max(mask[vi], w)
    return mask


def smooth_mask(mask: np.ndarray, floor: float = 0.0, power: float = 1.0) -> np.ndarray:
    if power != 1.0:
        mask = np.power(np.clip(mask, 0.0, 1.0), power)
    if floor > 0.0:
        mask = np.where(mask > floor, mask, 0.0)
    return mask.astype(np.float32)


def lateral_delta(mask: np.ndarray, rel_x: np.ndarray, side: np.ndarray, magnitude: float) -> np.ndarray:
    count = mask.shape[0]
    return (mask[:, None] * np.stack([side * rel_x * magnitude, np.zeros(count), np.zeros(count)], axis=1)).astype(
        np.float32
    )


def normal_delta(mask: np.ndarray, normals: np.ndarray, magnitude: float) -> np.ndarray:
    return (mask[:, None] * normals * magnitude).astype(np.float32)


def compute_regional_masks(
    positions: np.ndarray,
    joints: np.ndarray,
    weights: np.ndarray,
    joint_names: list[str],
) -> dict[str, np.ndarray]:
    head = vertex_influence_mask(positions.shape[0], joints, weights, joint_names, HEAD_JOINT_SUFFIXES)
    shoulder = vertex_influence_mask(positions.shape[0], joints, weights, joint_names, SHOULDER_JOINT_SUFFIXES)
    upper_arm = vertex_influence_mask(positions.shape[0], joints, weights, joint_names, ARM_UPPER_JOINT_SUFFIXES)
    lower_arm = vertex_influence_mask(positions.shape[0], joints, weights, joint_names, ARM_LOWER_JOINT_SUFFIXES)
    hand = vertex_influence_mask(positions.shape[0], joints, weights, joint_names, HAND_JOINT_SUFFIXES)
    spine = vertex_influence_mask(positions.shape[0], joints, weights, joint_names, SPINE_JOINT_SUFFIXES)
    pelvis = vertex_influence_mask(positions.shape[0], joints, weights, joint_names, PELVIS_JOINT_SUFFIXES)
    thigh = vertex_influence_mask(positions.shape[0], joints, weights, joint_names, THIGH_JOINT_SUFFIXES)
    calf = vertex_influence_mask(positions.shape[0], joints, weights, joint_names, CALF_JOINT_SUFFIXES)
    foot = vertex_influence_mask(positions.shape[0], joints, weights, joint_names, FOOT_JOINT_SUFFIXES)

    y = positions[:, 1]
    y_span = max(float(y.max() - y.min()), 1e-4)
    y_norm = (y - y.min()) / y_span

    shoulder_band = np.clip((y_norm - 0.62) / 0.12, 0.0, 1.0)
    torso_band = np.clip(1.0 - np.abs(y_norm - 0.52) / 0.18, 0.0, 1.0)
    hip_band = np.clip(1.0 - np.abs(y_norm - 0.42) / 0.10, 0.0, 1.0)
    leg_band = np.clip((0.40 - y_norm) / 0.32, 0.0, 1.0)

    shoulders = smooth_mask(shoulder + upper_arm * 0.35 * shoulder_band, floor=0.05, power=1.2)
    shoulders *= np.clip(1.0 - head * 0.85, 0.0, 1.0)

    torso = smooth_mask(spine * (0.55 + 0.45 * torso_band), floor=0.04, power=1.0)
    torso *= np.clip(1.0 - shoulder * 0.40 - pelvis * 0.20, 0.0, 1.0)
    torso *= np.clip(1.0 - head * 0.45, 0.0, 1.0)

    arms = smooth_mask((upper_arm * 0.85 + lower_arm * 0.95) * (1.0 - hand), floor=0.05, power=1.1)
    arms *= np.clip(1.0 - shoulders * 0.12, 0.0, 1.0)

    hips = smooth_mask(pelvis * 0.9 + thigh * 0.25 * hip_band, floor=0.05, power=1.15)
    hips *= np.clip(1.0 - torso * 0.30, 0.0, 1.0)

    legs = smooth_mask((thigh * 0.85 + calf * 0.95) * leg_band * (1.0 - foot), floor=0.05, power=1.1)
    legs *= np.clip(1.0 - hips * 0.20, 0.0, 1.0)

    return {
        "shoulders": shoulders,
        "torso": torso,
        "arms": arms,
        "hips": hips,
        "legs": legs,
    }


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

        regional = compute_regional_masks(positions, joints, weights, joint_names)
        rel_x_norm = np.clip(np.abs(rel_x) / (np.percentile(np.abs(rel_x), 90) + 1e-4), 0.0, 1.0)

        shoulders = regional["shoulders"] * (0.65 + 0.35 * rel_x_norm)
        torso = regional["torso"] * (0.70 + 0.30 * rel_x_norm)
        hips = regional["hips"] * (0.65 + 0.35 * rel_x_norm)

        scale = REGIONAL_MAGNITUDE_SCALE
        deltas["shoulders_broad"] = lateral_delta(shoulders, np.ones(count), side, 0.030 * scale)
        deltas["shoulders_narrow"] = lateral_delta(shoulders, np.ones(count), side, -0.026 * scale)
        deltas["torso_broad"] = lateral_delta(torso, np.ones(count), side, 0.038 * scale)
        deltas["torso_narrow"] = lateral_delta(torso, np.ones(count), side, -0.034 * scale)
        deltas["arms_thick"] = normal_delta(regional["arms"], normals, 0.020 * scale)
        deltas["arms_thin"] = normal_delta(regional["arms"], normals, -0.016 * scale)
        deltas["hips_broad"] = lateral_delta(hips, np.ones(count), side, 0.026 * scale)
        deltas["hips_narrow"] = lateral_delta(hips, np.ones(count), side, -0.022 * scale)
        deltas["legs_thick"] = normal_delta(regional["legs"], normals, 0.018 * scale)
        deltas["legs_thin"] = normal_delta(regional["legs"], normals, -0.015 * scale)

    head_center = positions[head_mask > 0.1].mean(axis=0) if np.any(head_mask > 0.1) else positions.mean(axis=0)
    head_vec = positions - head_center
    head_large = head_mask[:, None] * head_vec * 0.12
    head_small = head_mask[:, None] * head_vec * -0.10
    deltas["head_large"] = head_large.astype(np.float32)
    deltas["head_small"] = head_small.astype(np.float32)

    return deltas


def smooth_transition_deltas(
    deltas: dict[str, np.ndarray],
    regional_masks: dict[str, np.ndarray],
    neighbors: list[list[int]],
) -> dict[str, np.ndarray]:
    """Blend regional deltas toward neighbors in low-mask transition bands."""
    regional_names = set(REGIONAL_MORPH_TARGET_NAMES)
    out = {name: arr.copy() for name, arr in deltas.items()}
    transition = np.zeros_like(next(iter(regional_masks.values())))
    for mask in regional_masks.values():
        transition = np.maximum(transition, mask)
    transition = np.clip(transition, 0.0, 1.0)
    edge = (transition > 0.05) & (transition < 0.85)
    for name in regional_names:
        if name not in out:
            continue
        arr = out[name]
        for _ in range(DELTA_SMOOTH_ITERS):
            nxt = arr.copy()
            for vi, nbrs in enumerate(neighbors):
                if not edge[vi] or not nbrs:
                    continue
                nxt[vi] = (1.0 - DELTA_SMOOTH_ALPHA) * arr[vi] + DELTA_SMOOTH_ALPHA * np.mean(
                    arr[nbrs], axis=0
                )
            arr = nxt
        out[name] = arr
    return out


def find_seam_groups(primitives: list[tuple[np.ndarray, dict[str, np.ndarray]]]) -> list[list[tuple[int, int]]]:
    groups: list[list[tuple[int, int]]] = []
    assigned: set[tuple[int, int]] = set()
    positions = [p[0] for p in primitives]
    for i in range(len(primitives)):
        step = max(1, len(positions[i]) // 2500)
        for vi in range(0, len(positions[i]), step):
            if (i, vi) in assigned:
                continue
            for j in range(i + 1, len(primitives)):
                d = np.linalg.norm(positions[j] - positions[i][vi], axis=1)
                jj = int(np.argmin(d))
                if d[jj] > SEAM_VERTEX_EPS:
                    continue
                group = []
                for key in ((i, vi), (j, jj)):
                    if key not in assigned:
                        group.append(key)
                        assigned.add(key)
                if len(group) >= 2:
                    groups.append(group)
    return groups


def synchronize_seam_deltas(
    primitives: list[tuple[np.ndarray, dict[str, np.ndarray]]],
    target_names: tuple[str, ...],
) -> None:
    groups = find_seam_groups(primitives)
    for group in groups:
        for target in target_names:
            samples = []
            for prim_idx, vert_idx in group:
                delta = primitives[prim_idx][1].get(target)
                if delta is not None:
                    samples.append(delta[vert_idx])
            if not samples:
                continue
            mean = np.mean(samples, axis=0).astype(np.float32)
            for prim_idx, vert_idx in group:
                primitives[prim_idx][1][target][vert_idx] = mean


def set_bevy_mesh_morph_target_names(mesh: dict) -> None:
    """Bevy 0.18 glTF loader reads names from mesh extras, not per-target `name` fields."""
    mesh["extras"] = {"targetNames": list(MORPH_TARGET_NAMES)}


def author_morph_targets(js: dict, blob: bytearray) -> None:
    """Mutate glTF JSON + BIN in place, adding morph targets to skinned meshes."""
    pending: list[tuple[dict, str, int, np.ndarray, dict[str, np.ndarray]]] = []
    primitive_records: list[tuple[np.ndarray, dict[str, np.ndarray]]] = []

    for mesh_idx, mesh in enumerate(js.get("meshes", [])):
        mesh_name = mesh.get("name", f"mesh_{mesh_idx}")
        for primitive in mesh.get("primitives", []):
            attrs = primitive.get("attributes", {})
            if "JOINTS_0" not in attrs or "POSITION" not in attrs:
                continue
            skin_idx = 0
            for node in js.get("nodes", []):
                if node.get("mesh") == mesh_idx and node.get("skin") is not None:
                    skin_idx = node["skin"]
                    break
            positions = read_accessor(js, blob, attrs["POSITION"])
            normals = read_accessor(js, blob, attrs["NORMAL"])
            joints = read_accessor(js, blob, attrs["JOINTS_0"])
            weights = read_accessor(js, blob, attrs["WEIGHTS_0"])
            if weights.shape[1] > 4:
                weights = weights[:, :4]
            if joints.shape[1] > 4:
                joints = joints[:, :4]
            joint_names = skin_joint_names(js, skin_idx)
            deltas = compute_morph_deltas(
                positions, normals, joints, weights, joint_names, mesh_name
            )
            regional_masks = compute_regional_masks(
                positions, joints, weights, joint_names
            )
            tris = triangle_indices(js, blob, primitive)
            neighbors = vertex_neighbors(tris, len(positions))
            deltas = smooth_transition_deltas(deltas, regional_masks, neighbors)
            pending.append((primitive, mesh_name, skin_idx, positions, deltas))
            primitive_records.append((positions.astype(np.float32), deltas))

    synchronize_seam_deltas(primitive_records, MORPH_TARGET_NAMES)

    meshes_with_morphs: set[int] = set()
    for mesh_idx, mesh in enumerate(js.get("meshes", [])):
        for primitive in mesh.get("primitives", []):
            for pending_prim, _, _, _, deltas in pending:
                if pending_prim is not primitive:
                    continue
                targets = []
                for target_name in MORPH_TARGET_NAMES:
                    acc = write_accessor(js, blob, deltas[target_name], "VEC3")
                    targets.append({"POSITION": acc, "name": target_name})
                primitive["targets"] = targets
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
