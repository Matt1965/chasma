#!/usr/bin/env python3
"""Bake equipment visual fit into skinned equipment GLBs.

Reads authored fit_scale / fit_offset per equipment asset, inflates bind-pose
vertices uniformly around the mesh centroid, then scales morph target deltas to
match. Skinned overlays consume this offline bake; runtime Transform scale would
desync joint-bound meshes from the live unit skeleton.

After running this script, re-run author_equipment_morph_targets.py so CG5 morph
deltas are re-transferred onto the inflated bind pose.
"""

from __future__ import annotations

import json
import struct
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[1]
ASSETS = REPO / "assets"

# (fit_scale, fit_offset_xyz meters)
EQUIPMENT_FIT_CONFIG: dict[tuple[str, str], tuple[float, tuple[float, float, float]]] = {
    ("human_male", "peasant_body"): (1.04, (0.0, 0.0, 0.0)),
    ("human_male", "peasant_arms"): (1.035, (0.0, 0.0, 0.0)),
    ("human_male", "peasant_legs"): (1.04, (0.0, 0.0, 0.0)),
    ("human_male", "ranger_body"): (1.04, (0.0, 0.0, 0.0)),
    ("human_male", "ranger_arms"): (1.035, (0.0, 0.0, 0.0)),
    ("human_male", "ranger_legs"): (1.04, (0.0, 0.0, 0.0)),
    ("human_male", "ranger_hood"): (1.03, (0.0, 0.004, 0.0)),
    ("human_female", "peasant_body"): (1.04, (0.0, 0.0, 0.0)),
    ("human_female", "peasant_arms"): (1.035, (0.0, 0.0, 0.0)),
    ("human_female", "peasant_legs"): (1.04, (0.0, 0.0, 0.0)),
    ("human_female", "ranger_body"): (1.04, (0.0, 0.0, 0.0)),
    ("human_female", "ranger_arms"): (1.035, (0.0, 0.0, 0.0)),
    ("human_female", "ranger_legs"): (1.04, (0.0, 0.0, 0.0)),
    ("human_female", "ranger_hood"): (1.03, (0.0, 0.004, 0.0)),
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
    atype = acc["type"]
    comps = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[atype]
    dtype = {5126: np.float32, 5121: np.uint8, 5123: np.uint16, 5125: np.uint32}[acc["componentType"]]
    stride = bv.get("byteStride", comps * np.dtype(dtype).itemsize)
    raw = blob[start : start + count * stride]
    arr = np.frombuffer(raw, dtype=dtype, count=count * comps)
    return arr.reshape(count, comps)


def write_accessor(js: dict, blob: bytearray, values: np.ndarray, atype: str) -> int:
    flat = np.asarray(values, dtype=np.float32).reshape(-1)
    raw = flat.tobytes()
    byte_offset = len(blob)
    blob.extend(raw)
    bv_idx = len(js["bufferViews"])
    js["bufferViews"].append({"buffer": 0, "byteOffset": byte_offset, "byteLength": len(raw)})
    comps = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[atype]
    count = len(flat) // comps
    accessor = {
        "bufferView": bv_idx,
        "componentType": 5126,
        "count": count,
        "type": atype,
    }
    if atype == "VEC3" and count > 0:
        vectors = flat.reshape(count, 3)
        accessor["min"] = vectors.min(axis=0).tolist()
        accessor["max"] = vectors.max(axis=0).tolist()
    acc_idx = len(js["accessors"])
    js["accessors"].append(accessor)
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
            count = js["accessors"][attrs["POSITION"]]["count"]
            if best is None or count > best[2]:
                best = (mesh_idx, primitive, count)
    if best is None:
        return None
    return best[0], best[1]


def apply_fit(positions: np.ndarray, fit_scale: float, fit_offset: tuple[float, float, float]) -> np.ndarray:
    center = positions.mean(axis=0)
    offset = np.asarray(fit_offset, dtype=np.float32)
    return center + (positions - center) * fit_scale + offset


def author_fit_file(path: Path, fit_scale: float, fit_offset: tuple[float, float, float]) -> None:
    if abs(fit_scale - 1.0) < 1e-6 and all(abs(v) < 1e-6 for v in fit_offset):
        print(f"skip {path.name}: identity fit")
        return
    js, blob = load_glb(path)
    mesh = primary_skinned_primitive(js)
    if mesh is None:
        raise ValueError(f"no skinned primitive in {path}")
    _, primitive = mesh
    pos_acc = primitive["attributes"]["POSITION"]
    positions = read_accessor(js, blob, pos_acc).astype(np.float32)
    fitted = apply_fit(positions, fit_scale, fit_offset)
    primitive["attributes"]["POSITION"] = write_accessor(js, blob, fitted, "VEC3")
    for target in primitive.get("targets", []):
        if "POSITION" not in target:
            continue
        deltas = read_accessor(js, blob, target["POSITION"]).astype(np.float32)
        target["POSITION"] = write_accessor(js, blob, deltas * fit_scale, "VEC3")
    save_glb(path, js, blob)
    print(f"baked fit scale={fit_scale:.3f} offset={fit_offset} on {path}")


def main() -> None:
    targets = sys.argv[1:]
    if targets:
        for target in targets:
            path = Path(target)
            unit_key = path.parent.name
            asset_name = path.stem
            fit_scale, fit_offset = EQUIPMENT_FIT_CONFIG.get((unit_key, asset_name), (1.0, (0.0, 0.0, 0.0)))
            author_fit_file(path, fit_scale, fit_offset)
        return
    for unit_key, asset_name in sorted(EQUIPMENT_FIT_CONFIG):
        path = ASSETS / "items" / "equipment" / unit_key / f"{asset_name}.glb"
        fit_scale, fit_offset = EQUIPMENT_FIT_CONFIG[(unit_key, asset_name)]
        author_fit_file(path, fit_scale, fit_offset)


if __name__ == "__main__":
    main()
