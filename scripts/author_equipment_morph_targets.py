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

ALL_TARGET_NAMES: tuple[str, ...] = (
    "build_broad",
    "build_narrow",
    "fat_soft",
    "muscle_define",
    "head_large",
    "head_small",
)

# Technical targets authored per equipment asset basename.
EQUIPMENT_TARGET_CONFIG: dict[str, tuple[str, ...]] = {
    "peasant_body": ("build_broad", "build_narrow", "fat_soft", "muscle_define"),
    "peasant_arms": ("build_broad", "build_narrow", "fat_soft", "muscle_define"),
    "peasant_legs": ("build_broad", "build_narrow", "fat_soft", "muscle_define"),
    "peasant_feet": (),
    "ranger_body": ("build_broad", "build_narrow", "fat_soft", "muscle_define"),
    "ranger_arms": ("build_broad", "build_narrow", "fat_soft", "muscle_define"),
    "ranger_legs": ("build_broad", "build_narrow", "fat_soft", "muscle_define"),
    "ranger_feet": (),
    "ranger_hood": ("head_large", "head_small"),
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
    names = extras.get("targetNames", ALL_TARGET_NAMES)
    deltas: dict[str, np.ndarray] = {}
    for index, target in enumerate(targets):
        name = names[index] if index < len(names) else f"target_{index}"
        acc = target["POSITION"]
        deltas[name] = read_accessor(js, blob, acc).astype(np.float32)
    return positions, deltas


def nearest_body_indices(armor_positions: np.ndarray, body_positions: np.ndarray) -> np.ndarray:
    indices = np.empty(armor_positions.shape[0], dtype=np.int32)
    for i, vertex in enumerate(armor_positions):
        dists = np.sum((body_positions - vertex) ** 2, axis=1)
        indices[i] = int(np.argmin(dists))
    return indices


def transfer_deltas(
    armor_positions: np.ndarray,
    body_positions: np.ndarray,
    body_deltas: dict[str, np.ndarray],
    target_names: tuple[str, ...],
) -> dict[str, np.ndarray]:
    nearest = nearest_body_indices(armor_positions, body_positions)
    out: dict[str, np.ndarray] = {}
    for name in target_names:
        source = body_deltas.get(name)
        if source is None:
            raise KeyError(f"body missing morph target `{name}`")
        out[name] = source[nearest].astype(np.float32)
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
    armor_positions = read_accessor(armor_js, armor_blob, armor_primitive["attributes"]["POSITION"])
    transferred = transfer_deltas(armor_positions, body_positions, body_deltas, target_names)

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
