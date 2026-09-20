#!/usr/bin/env python3
"""Shared GLB geometry helpers for CG9 offline authoring."""

from __future__ import annotations

import json
import struct
from dataclasses import dataclass
from pathlib import Path

import numpy as np

COMPONENT_TYPES = {
    5120: np.int8,
    5121: np.uint8,
    5122: np.int16,
    5123: np.uint16,
    5125: np.uint32,
    5126: np.float32,
}


def load_glb(path: Path) -> tuple[dict, bytearray]:
    data = path.read_bytes()
    off = 12
    cl, _ = struct.unpack_from("<II", data, off)
    off += 8
    js = json.loads(data[off : off + cl])
    blob = bytearray()
    off = 12
    while off < len(data):
        cl, ty = struct.unpack_from("<II", data, off)
        off += 8
        chunk = data[off : off + cl]
        off += cl
        if ty == 0x004E4942:
            blob = bytearray(chunk)
    return js, blob


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


def read_accessor(js: dict, blob: bytearray, accessor_idx: int) -> np.ndarray:
    acc = js["accessors"][accessor_idx]
    bv = js["bufferViews"][acc["bufferView"]]
    start = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    count = acc["count"]
    ctype = acc["componentType"]
    atype = acc["type"]
    comps = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[atype]
    dtype = COMPONENT_TYPES[ctype]
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
    js["bufferViews"].append(
        {"buffer": 0, "byteOffset": byte_offset, "byteLength": len(raw)}
    )
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


@dataclass
class SkinnedPrimitive:
    mesh_idx: int
    prim_idx: int
    mesh_name: str
    primitive: dict
    positions: np.ndarray
    normals: np.ndarray
    target_names: list[str]
    target_deltas: dict[str, np.ndarray]


def mesh_target_names(mesh: dict) -> list[str]:
    extras = mesh.get("extras", {})
    if isinstance(extras, str):
        extras = json.loads(extras) if extras else {}
    return list(extras.get("targetNames", []))


def primary_skinned_primitive(js: dict, blob: bytearray) -> SkinnedPrimitive | None:
    best: tuple[int, int, dict, int] | None = None
    for mesh_idx, mesh in enumerate(js.get("meshes", [])):
        for prim_idx, primitive in enumerate(mesh.get("primitives", [])):
            attrs = primitive.get("attributes", {})
            if "JOINTS_0" not in attrs or "POSITION" not in attrs:
                continue
            count = js["accessors"][attrs["POSITION"]]["count"]
            if best is None or count > best[3]:
                best = (mesh_idx, prim_idx, primitive, count)
    if best is None:
        return None
    mesh_idx, prim_idx, primitive, _ = best
    mesh = js["meshes"][mesh_idx]
    names = mesh_target_names(mesh)
    positions = read_accessor(js, blob, primitive["attributes"]["POSITION"]).astype(np.float32)
    normals = read_accessor(js, blob, primitive["attributes"]["NORMAL"]).astype(np.float32)
    deltas: dict[str, np.ndarray] = {}
    for ti, target in enumerate(primitive.get("targets", [])):
        name = names[ti] if ti < len(names) else f"target_{ti}"
        deltas[name] = read_accessor(js, blob, target["POSITION"]).astype(np.float32)
    return SkinnedPrimitive(
        mesh_idx=mesh_idx,
        prim_idx=prim_idx,
        mesh_name=mesh.get("name", f"mesh_{mesh_idx}"),
        primitive=primitive,
        positions=positions,
        normals=normals,
        target_names=names,
        target_deltas=deltas,
    )


def all_skinned_primitives(js: dict, blob: bytearray) -> list[SkinnedPrimitive]:
    out: list[SkinnedPrimitive] = []
    for mesh_idx, mesh in enumerate(js.get("meshes", [])):
        names = mesh_target_names(mesh)
        for prim_idx, primitive in enumerate(mesh.get("primitives", [])):
            attrs = primitive.get("attributes", {})
            if "JOINTS_0" not in attrs or "POSITION" not in attrs:
                continue
            positions = read_accessor(js, blob, attrs["POSITION"]).astype(np.float32)
            normals = read_accessor(js, blob, attrs["NORMAL"]).astype(np.float32)
            deltas: dict[str, np.ndarray] = {}
            for ti, target in enumerate(primitive.get("targets", [])):
                name = names[ti] if ti < len(names) else f"target_{ti}"
                deltas[name] = read_accessor(js, blob, target["POSITION"]).astype(np.float32)
            out.append(
                SkinnedPrimitive(
                    mesh_idx=mesh_idx,
                    prim_idx=prim_idx,
                    mesh_name=mesh.get("name", f"mesh_{mesh_idx}"),
                    primitive=primitive,
                    positions=positions,
                    normals=normals,
                    target_names=names,
                    target_deltas=deltas,
                )
            )
    return out


def triangle_indices(js: dict, blob: bytearray, primitive: dict) -> np.ndarray:
    if "indices" not in primitive:
        count = js["accessors"][primitive["attributes"]["POSITION"]]["count"]
        return np.arange(count, dtype=np.int32).reshape(-1, 3)
    idx = read_accessor(js, blob, primitive["indices"]).astype(np.int32).reshape(-1)
    return idx.reshape(-1, 3)


def vertex_neighbors(triangles: np.ndarray, count: int) -> list[list[int]]:
    neighbors: list[set[int]] = [set() for _ in range(count)]
    for a, b, c in triangles:
        neighbors[a].update((b, c))
        neighbors[b].update((a, c))
        neighbors[c].update((a, b))
    return [sorted(n) for n in neighbors]


def apply_morph_weights(
    positions: np.ndarray,
    target_deltas: dict[str, np.ndarray],
    weights: dict[str, float],
) -> np.ndarray:
    out = positions.copy()
    for name, weight in weights.items():
        if abs(weight) < 1e-8:
            continue
        delta = target_deltas.get(name)
        if delta is not None:
            out += delta * weight
    return out


def closest_point_on_triangle(
    points: np.ndarray, tri_a: np.ndarray, tri_b: np.ndarray, tri_c: np.ndarray
) -> tuple[np.ndarray, np.ndarray]:
    """Return closest points on triangle for each query point and squared distances."""
    ab = tri_b - tri_a
    ac = tri_c - tri_a
    ap = points[:, None, :] - tri_a[None, :, :]
    d1 = np.sum(ab[None, :, :] * ap, axis=2)
    d2 = np.sum(ac[None, :, :] * ap, axis=2)
    # Simplified: use vertex closest for batches (good enough for clearance sampling)
    verts = np.stack([tri_a, tri_b, tri_c], axis=1)
    dists = np.sum((points[:, None, None, :] - verts[None, :, :, :]) ** 2, axis=3)
    flat_idx = np.argmin(dists.reshape(len(points), -1), axis=1)
    tri_idx = flat_idx // 3
    vert_idx = flat_idx % 3
    closest = verts[tri_idx, vert_idx]
    return closest, dists.reshape(len(points), -1).min(axis=1)


class BodySampleGrid:
    """Uniform grid index for nearby body vertex queries."""

    def __init__(self, verts: np.ndarray, cell_size: float = 0.02):
        self.verts = verts.astype(np.float32)
        self.cell_size = float(cell_size)
        self.grid: dict[tuple[int, int, int], list[int]] = {}
        for index, vert in enumerate(self.verts):
            key = tuple(np.floor(vert / self.cell_size).astype(np.int32))
            self.grid.setdefault(key, []).append(index)

    def nearby_indices(self, point: np.ndarray, radius: float) -> np.ndarray:
        cell = np.floor(point / self.cell_size).astype(np.int32)
        span = int(np.ceil(radius / self.cell_size)) + 1
        collected: list[int] = []
        for dx in range(-span, span + 1):
            for dy in range(-span, span + 1):
                for dz in range(-span, span + 1):
                    key = (int(cell[0] + dx), int(cell[1] + dy), int(cell[2] + dz))
                    collected.extend(self.grid.get(key, []))
        return np.asarray(collected, dtype=np.int32)


def write_skinned_primitive(
    js: dict, blob: bytearray, equip: SkinnedPrimitive
) -> None:
    equip.primitive["attributes"]["POSITION"] = write_accessor(
        js, blob, equip.positions, "VEC3"
    )
    for target_index, target in enumerate(equip.primitive.get("targets", [])):
        name = (
            equip.target_names[target_index]
            if target_index < len(equip.target_names)
            else f"target_{target_index}"
        )
        target["POSITION"] = write_accessor(
            js, blob, equip.target_deltas[name], "VEC3"
        )


def body_surface_kdtree(body_positions: np.ndarray, body_normals: np.ndarray):
    """Simple chunked nearest-body lookup using vertex samples."""

    class BodySurfaceQuery:
        def __init__(self, positions: np.ndarray, normals: np.ndarray):
            self.positions = positions
            self.normals = normals
            chunk = max(1, len(positions) // 64)
            self.centers = positions[::chunk]
            self.chunk = chunk

        def query(self, points: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
            closest = np.empty_like(points)
            dist_sq = np.empty(len(points), dtype=np.float64)
            normal = np.empty_like(points)
            for i, point in enumerate(points):
                local = self.centers - point
                chunk_idx = int(np.argmin(np.sum(local * local, axis=1)))
                start = chunk_idx * self.chunk
                end = min(len(self.positions), start + self.chunk * 2)
                window = self.positions[start:end]
                d = np.sum((window - point) ** 2, axis=1)
                j = int(np.argmin(d))
                closest[i] = window[j]
                dist_sq[i] = d[j]
                normal[i] = self.normals[start + j]
            return closest, np.sqrt(dist_sq), normal

    return BodySurfaceQuery(body_positions, body_normals)
