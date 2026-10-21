"""Fold glTF skin influence sets into one 4-wide, normalized `JOINTS_0` / `WEIGHTS_0`.

FBX2glTF emits every influence it finds in the FBX (up to 8 per vertex) as
`JOINTS_0/WEIGHTS_0` + `JOINTS_1/WEIGHTS_1`. Bevy reads set 0 only, so any vertex
with influences in set 1 has `sum(WEIGHTS_0) < 1`, and linear-blend skinning then
displaces it by `(1 - sum) * world_position` - a streak toward the world origin whose
length grows with distance from the origin (invisible when evaluated at the origin).

This step is a conversion, not a workaround: the renderer supports exactly four
influences per vertex, so the four largest are kept and renormalized (the same policy
Unity "4 bones" and Blender's exporter apply).
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path
from typing import Any

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from glb_sanitize_animations import load_glb, write_glb  # noqa: E402

MAX_INFLUENCES = 4
COMPONENT_DTYPE = {5120: np.int8, 5121: np.uint8, 5122: np.int16, 5123: np.uint16, 5125: np.uint32, 5126: np.float32}
TYPE_COMPONENTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


def read_accessor(gltf: dict, blob: bytes, index: int) -> np.ndarray:
    acc = gltf["accessors"][index]
    view = gltf["bufferViews"][acc["bufferView"]]
    dtype = COMPONENT_DTYPE[acc["componentType"]]
    comps = TYPE_COMPONENTS[acc["type"]]
    start = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    item = comps * np.dtype(dtype).itemsize
    stride = view.get("byteStride", item) or item
    if stride != item:
        raw = np.frombuffer(blob, dtype=np.uint8, count=stride * (acc["count"] - 1) + item, offset=start)
        rows = np.lib.stride_tricks.as_strided(raw, shape=(acc["count"], item), strides=(stride, 1))
        arr = np.ascontiguousarray(rows).view(dtype).reshape(acc["count"], comps)
    else:
        arr = np.frombuffer(blob, dtype=dtype, count=acc["count"] * comps, offset=start).reshape(acc["count"], comps)
    if acc.get("normalized"):
        arr = arr.astype(np.float64) / np.iinfo(dtype).max
    return arr


def append_accessor(gltf: dict, blob: bytearray, data: np.ndarray, component_type: int, type_name: str) -> int:
    while len(blob) % 4:
        blob.append(0)
    offset = len(blob)
    payload = np.ascontiguousarray(data.astype(COMPONENT_DTYPE[component_type])).tobytes()
    blob.extend(payload)
    gltf["bufferViews"].append({"buffer": 0, "byteOffset": offset, "byteLength": len(payload), "target": 34962})
    gltf["accessors"].append(
        {
            "bufferView": len(gltf["bufferViews"]) - 1,
            "componentType": component_type,
            "count": int(data.shape[0]),
            "type": type_name,
        }
    )
    return len(gltf["accessors"]) - 1


def _influence_sets(attributes: dict) -> list[int]:
    sets = []
    k = 0
    while f"JOINTS_{k}" in attributes:
        if f"WEIGHTS_{k}" not in attributes:
            raise ValueError(f"JOINTS_{k} present without WEIGHTS_{k}")
        sets.append(k)
        k += 1
    return sets


def fold_primitive(gltf: dict, blob: bytearray, primitive: dict, joint_count: int | None) -> dict[str, Any]:
    attrs = primitive["attributes"]
    sets = _influence_sets(attrs)
    joints = np.concatenate([read_accessor(gltf, blob, attrs[f"JOINTS_{k}"]).astype(np.int64) for k in sets], axis=1)
    weights = np.concatenate([read_accessor(gltf, blob, attrs[f"WEIGHTS_{k}"]).astype(np.float64) for k in sets], axis=1)
    vertex_count = joints.shape[0]

    # Merge duplicate joint references within a vertex (sum their weights).
    out_j = np.zeros((vertex_count, MAX_INFLUENCES), dtype=np.int64)
    out_w = np.zeros((vertex_count, MAX_INFLUENCES), dtype=np.float64)
    dropped_mass = np.zeros(vertex_count, dtype=np.float64)
    changed = 0
    for v in range(vertex_count):
        acc: dict[int, float] = {}
        for j, w in zip(joints[v], weights[v]):
            if w <= 0.0:
                continue
            acc[int(j)] = acc.get(int(j), 0.0) + float(w)
        ranked = sorted(acc.items(), key=lambda kv: kv[1], reverse=True)
        kept = ranked[:MAX_INFLUENCES]
        dropped_mass[v] = sum(w for _, w in ranked[MAX_INFLUENCES:])
        total = sum(w for _, w in kept)
        if total <= 0.0:
            kept = [(0, 1.0)]
            total = 1.0
        for slot, (j, w) in enumerate(kept):
            out_j[v, slot] = j
            out_w[v, slot] = w / total
        original_sum = float(weights[v].sum())
        if len(sets) > 1 or abs(original_sum - 1.0) > 1e-4 or len(ranked) != len(kept):
            changed += 1

    if joint_count is not None and out_j.max(initial=0) >= joint_count:
        raise ValueError(f"joint index {int(out_j.max())} out of range for skin with {joint_count} joints")

    for k in sets:
        del attrs[f"JOINTS_{k}"]
        del attrs[f"WEIGHTS_{k}"]
    attrs["JOINTS_0"] = append_accessor(gltf, blob, out_j, 5123, "VEC4")
    attrs["WEIGHTS_0"] = append_accessor(gltf, blob, out_w.astype(np.float32), 5126, "VEC4")

    return {
        "vertices": int(vertex_count),
        "input_sets": len(sets),
        "input_weight_sum_min": float(weights.sum(axis=1).min()),
        "input_weight_sum_max": float(weights.sum(axis=1).max()),
        "vertices_changed": int(changed),
        "vertices_with_dropped_influences": int((dropped_mass > 0).sum()),
        "max_dropped_weight_mass": float(dropped_mass.max()),
    }


def normalize_skin_influences(gltf: dict, blob: bytes) -> tuple[bytes, dict[str, Any]]:
    """Rewrite every skinned primitive to a single normalized 4-influence set."""
    blob_out = bytearray(blob)
    mesh_skin: dict[int, int] = {}
    for node in gltf["nodes"]:
        if node.get("mesh") is not None and node.get("skin") is not None:
            mesh_skin[node["mesh"]] = node["skin"]
    report: dict[str, Any] = {"primitives": []}
    for mesh_index, mesh in enumerate(gltf.get("meshes", [])):
        for prim_index, primitive in enumerate(mesh.get("primitives", [])):
            if "JOINTS_0" not in primitive["attributes"]:
                continue
            skin_index = mesh_skin.get(mesh_index)
            joint_count = len(gltf["skins"][skin_index]["joints"]) if skin_index is not None else None
            entry = fold_primitive(gltf, blob_out, primitive, joint_count)
            entry.update({"mesh": mesh.get("name"), "mesh_index": mesh_index, "primitive": prim_index})
            report["primitives"].append(entry)
    gltf["buffers"][0]["byteLength"] = len(blob_out)
    report["max_input_sets"] = max((p["input_sets"] for p in report["primitives"]), default=0)
    report["min_input_weight_sum"] = min((p["input_weight_sum_min"] for p in report["primitives"]), default=1.0)
    return bytes(blob_out), report


def normalize_glb_skin_influences(path: Path, out: Path | None = None) -> dict[str, Any]:
    gltf, blob = load_glb(path)
    blob, report = normalize_skin_influences(gltf, blob)
    write_glb(out or path, gltf, blob)
    return report


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("glb", type=Path)
    ap.add_argument("--out", type=Path, default=None)
    args = ap.parse_args()
    print(json.dumps(normalize_glb_skin_influences(args.glb, args.out), indent=2))


if __name__ == "__main__":
    main()
