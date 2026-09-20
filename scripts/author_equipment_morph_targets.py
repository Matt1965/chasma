#!/usr/bin/env python3
"""Author CG5 morph targets on Chasma skinned equipment GLBs.

Transfers morph deltas from the corresponding human body GLB via nearest-surface
spatial mapping. Each equipment piece receives only the technical targets it needs.
"""

from __future__ import annotations

import json
import struct
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[1]
ASSETS = REPO / "assets"

CG2_TORSO_TARGETS: tuple[str, ...] = (
    "build_broad",
    "build_narrow",
    "fat_soft",
    "muscle_define",
)

REGIONAL_BODY_TARGETS: tuple[str, ...] = (
    "shoulders_broad",
    "shoulders_narrow",
    "torso_broad",
    "torso_narrow",
    "hips_broad",
    "hips_narrow",
)

REGIONAL_ARMS_TARGETS: tuple[str, ...] = (
    "arms_thick",
    "arms_thin",
)

REGIONAL_LEGS_TARGETS: tuple[str, ...] = (
    "legs_thick",
    "legs_thin",
)

HEAD_TARGETS: tuple[str, ...] = (
    "head_large",
    "head_small",
)

BODY_EQUIPMENT_TARGETS: tuple[str, ...] = CG2_TORSO_TARGETS + REGIONAL_BODY_TARGETS
ARMS_EQUIPMENT_TARGETS: tuple[str, ...] = CG2_TORSO_TARGETS + REGIONAL_ARMS_TARGETS
# Leg meshes include upper-thigh/waist geometry; hips targets follow that coverage.
LEGS_EQUIPMENT_TARGETS: tuple[str, ...] = (
    CG2_TORSO_TARGETS + REGIONAL_LEGS_TARGETS + ("hips_broad", "hips_narrow")
)

ALL_BODY_TARGET_NAMES: tuple[str, ...] = (
    CG2_TORSO_TARGETS
    + HEAD_TARGETS
    + REGIONAL_BODY_TARGETS
    + REGIONAL_ARMS_TARGETS
    + REGIONAL_LEGS_TARGETS
)

# Technical targets authored per equipment asset basename.
EQUIPMENT_TARGET_CONFIG: dict[str, tuple[str, ...]] = {
    "peasant_body": BODY_EQUIPMENT_TARGETS,
    "peasant_arms": ARMS_EQUIPMENT_TARGETS,
    "peasant_legs": LEGS_EQUIPMENT_TARGETS,
    "peasant_feet": (),
    "ranger_body": BODY_EQUIPMENT_TARGETS,
    "ranger_arms": ARMS_EQUIPMENT_TARGETS,
    "ranger_legs": LEGS_EQUIPMENT_TARGETS,
    "ranger_feet": (),
    "ranger_hood": HEAD_TARGETS,
}


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


def save_glb(path: Path, js: dict, blob: bytearray) -> None:
    if js.get("bufferViews"):
        js["buffers"] = [{"byteLength": len(blob)}]
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


def primary_skinned_primitive(js: dict) -> tuple[int, dict] | None:
    best: tuple[int, dict, int] | None = None
    for mesh_idx, mesh in enumerate(js.get("meshes", [])):
        for primitive in mesh.get("primitives", []):
            attrs = primitive.get("attributes", {})
            if "JOINTS_0" not in attrs or "POSITION" not in attrs:
                continue
            pos_acc = attrs["POSITION"]
            count = js["accessors"][pos_acc]["count"]
            if best is None or count > best[2]:
                best = (mesh_idx, primitive, count)
    if best is None:
        return None
    return best[0], best[1]


def read_morph_deltas(
    js: dict, blob: bytearray, mesh_idx: int, primitive: dict
) -> tuple[np.ndarray, dict[str, np.ndarray]]:
    positions = read_accessor(js, blob, primitive["attributes"]["POSITION"])
    targets = primitive.get("targets", [])
    if not targets:
        raise ValueError("body primitive has no morph targets")
    mesh = js["meshes"][mesh_idx]
    extras = mesh.get("extras", {})
    if isinstance(extras, str):
        extras = json.loads(extras)
    names = extras.get("targetNames", ALL_BODY_TARGET_NAMES)
    deltas: dict[str, np.ndarray] = {}
    for index, target in enumerate(targets):
        name = names[index] if index < len(names) else f"target_{index}"
        acc = target["POSITION"]
        deltas[name] = read_accessor(js, blob, acc).astype(np.float32)
    return positions, deltas


NEAREST_K = 16
ZERO_DELTA_EPS = 1e-6


def read_joints_and_weights(
    js: dict, blob: bytearray, primitive: dict
) -> tuple[np.ndarray, np.ndarray]:
    joints = read_accessor(js, blob, primitive["attributes"]["JOINTS_0"]).astype(np.int32)
    weights = read_accessor(js, blob, primitive["attributes"]["WEIGHTS_0"]).astype(np.float32)
    if weights.shape[1] != joints.shape[1]:
        raise ValueError("joint/weight component mismatch")
    weight_sums = np.sum(weights, axis=1, keepdims=True)
    weight_sums = np.where(weight_sums > 0.0, weight_sums, 1.0)
    weights = weights / weight_sums
    return joints, weights


def dominant_joint_indices(joints: np.ndarray, weights: np.ndarray) -> np.ndarray:
    best = np.argmax(weights, axis=1)
    return joints[np.arange(joints.shape[0]), best]


def skin_parent_map(js: dict, skin_index: int = 0) -> dict[int, int]:
    skin = js["skins"][skin_index]
    joint_nodes = skin["joints"]
    node_to_joint = {node: index for index, node in enumerate(joint_nodes)}
    parent: dict[int, int] = {}
    for joint_index, node_index in enumerate(joint_nodes):
        for child_node in js["nodes"][node_index].get("children", []):
            child_joint = node_to_joint.get(child_node)
            if child_joint is not None:
                parent[child_joint] = joint_index
    return parent


def joint_influence_delta_fallback(
    source: np.ndarray,
    body_joints: np.ndarray,
    body_weights: np.ndarray,
    weight_threshold: float = 0.01,
) -> dict[int, np.ndarray]:
    norms = np.linalg.norm(source, axis=1)
    fallback: dict[int, np.ndarray] = {}
    for joint in np.unique(body_joints):
        mask = np.any(
            (body_joints == joint) & (body_weights >= weight_threshold),
            axis=1,
        )
        candidates = np.flatnonzero(mask)
        if candidates.size == 0:
            continue
        best = candidates[np.argmax(norms[candidates])]
        if norms[best] > ZERO_DELTA_EPS:
            fallback[int(joint)] = source[best].astype(np.float32)
    return fallback


def resolve_joint_delta(
    joint: int,
    joint_fallback: dict[int, np.ndarray],
    parent_map: dict[int, int],
) -> np.ndarray | None:
    current = joint
    for _ in range(len(parent_map) + 1):
        delta = joint_fallback.get(current)
        if delta is not None:
            return delta
        parent = parent_map.get(current)
        if parent is None:
            return None
        current = parent
    return None


def nearest_body_indices(armor_positions: np.ndarray, body_positions: np.ndarray) -> np.ndarray:
    indices = np.empty(armor_positions.shape[0], dtype=np.int32)
    for i, vertex in enumerate(armor_positions):
        dists = np.sum((body_positions - vertex) ** 2, axis=1)
        indices[i] = int(np.argmin(dists))
    return indices


def nearest_body_indices_k(
    armor_positions: np.ndarray, body_positions: np.ndarray, k: int
) -> np.ndarray:
    k = min(k, len(body_positions))
    indices = np.empty((armor_positions.shape[0], k), dtype=np.int32)
    for i, vertex in enumerate(armor_positions):
        dists = np.sum((body_positions - vertex) ** 2, axis=1)
        indices[i] = np.argpartition(dists, k - 1)[:k]
    return indices


def transfer_deltas(
    armor_positions: np.ndarray,
    body_positions: np.ndarray,
    body_deltas: dict[str, np.ndarray],
    target_names: tuple[str, ...],
    armor_joints: np.ndarray | None = None,
    armor_weights: np.ndarray | None = None,
    body_joints: np.ndarray | None = None,
    body_weights: np.ndarray | None = None,
    body_parent_map: dict[int, int] | None = None,
) -> dict[str, np.ndarray]:
    nearest = nearest_body_indices(armor_positions, body_positions)
    nearest_k = nearest_body_indices_k(armor_positions, body_positions, NEAREST_K)
    out: dict[str, np.ndarray] = {}
    for name in target_names:
        source = body_deltas.get(name)
        if source is None:
            raise KeyError(f"body missing morph target `{name}`")
        transferred = source[nearest].astype(np.float32)
        norms = np.linalg.norm(transferred, axis=1)
        needs_fallback = norms <= ZERO_DELTA_EPS
        if not np.any(needs_fallback):
            out[name] = transferred
            continue
        joint_fallback = (
            joint_influence_delta_fallback(source, body_joints, body_weights)
            if body_joints is not None and body_weights is not None
            else {}
        )
        for vertex_index in np.flatnonzero(needs_fallback):
            candidates = source[nearest_k[vertex_index]]
            candidate_norms = np.linalg.norm(candidates, axis=1)
            best = int(np.argmax(candidate_norms))
            if candidate_norms[best] > ZERO_DELTA_EPS:
                transferred[vertex_index] = candidates[best]
                continue
            if armor_joints is not None and armor_weights is not None and body_parent_map is not None:
                influence_order = np.argsort(-armor_weights[vertex_index])
                for slot in influence_order:
                    weight = armor_weights[vertex_index, slot]
                    if weight < 0.01:
                        continue
                    joint = int(armor_joints[vertex_index, slot])
                    joint_delta = resolve_joint_delta(joint, joint_fallback, body_parent_map)
                    if joint_delta is not None:
                        transferred[vertex_index] = joint_delta
                        break
        out[name] = transferred
    return out


def set_mesh_target_names(mesh: dict, target_names: tuple[str, ...]) -> None:
    mesh["extras"] = {"targetNames": list(target_names)}


def author_equipment_file(body_path: Path, armor_path: Path, target_names: tuple[str, ...]) -> None:
    if not target_names:
        print(f"skip {armor_path.name}: no morph targets configured")
        return

    body_js, body_blob = load_glb(body_path)
    armor_js, armor_blob = load_glb(armor_path)

    body_mesh = primary_skinned_primitive(body_js)
    armor_mesh = primary_skinned_primitive(armor_js)
    if body_mesh is None or armor_mesh is None:
        raise ValueError(f"missing skinned primitive in {body_path} or {armor_path}")

    body_mesh_idx, body_primitive = body_mesh
    armor_mesh_idx, armor_primitive = armor_mesh
    body_positions, body_deltas = read_morph_deltas(
        body_js, body_blob, body_mesh_idx, body_primitive
    )
    body_joints, body_weights = read_joints_and_weights(body_js, body_blob, body_primitive)
    armor_positions = read_accessor(armor_js, armor_blob, armor_primitive["attributes"]["POSITION"])
    armor_joints, armor_weights = read_joints_and_weights(armor_js, armor_blob, armor_primitive)
    transferred = transfer_deltas(
        armor_positions,
        body_positions,
        body_deltas,
        target_names,
        armor_joints=armor_joints,
        armor_weights=armor_weights,
        body_joints=body_joints,
        body_weights=body_weights,
        body_parent_map=skin_parent_map(body_js),
    )

    targets = []
    for name in target_names:
        acc = write_accessor(armor_js, armor_blob, transferred[name], "VEC3")
        targets.append({"POSITION": acc, "name": name})
    armor_primitive["targets"] = targets
    set_mesh_target_names(armor_js["meshes"][armor_mesh_idx], target_names)
    save_glb(armor_path, armor_js, armor_blob)
    print(f"authored {len(target_names)} morph targets on {armor_path}")


def author_variant(unit_key: str, asset_name: str) -> None:
    body_path = ASSETS / "units" / f"{unit_key}.glb"
    armor_path = ASSETS / "items" / "equipment" / unit_key / f"{asset_name}.glb"
    target_names = EQUIPMENT_TARGET_CONFIG.get(asset_name, ())
    author_equipment_file(body_path, armor_path, target_names)


def main() -> None:
    targets = sys.argv[1:]
    if targets:
        for target in targets:
            path = Path(target)
            asset_name = path.stem
            unit_key = path.parent.name
            body_path = ASSETS / "units" / f"{unit_key}.glb"
            names = EQUIPMENT_TARGET_CONFIG.get(asset_name, ())
            author_equipment_file(body_path, path, names)
        return

    for unit_key in ("human_male", "human_female"):
        for asset_name in EQUIPMENT_TARGET_CONFIG:
            author_variant(unit_key, asset_name)


if __name__ == "__main__":
    main()
