#!/usr/bin/env python3
"""Audit inter-primitive seam gap stability for CG9 body morph targets."""

from __future__ import annotations

import json
import struct
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[1]
SEAM_EPS = 0.0015
MAX_SEAM_GAP_INCREASE_M = 0.002


def load_glb(path: Path):
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


def read_acc(js, blob, idx):
    acc = js["accessors"][idx]
    bv = js["bufferViews"][acc["bufferView"]]
    start = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    count = acc["count"]
    comps = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[acc["type"]]
    dtype = {5126: np.float32, 5121: np.uint8, 5123: np.uint16, 5125: np.uint32}[acc["componentType"]]
    stride = bv.get("byteStride", comps * np.dtype(dtype).itemsize)
    raw = blob[start : start + count * stride]
    return np.frombuffer(raw, dtype=dtype, count=count * comps).reshape(count, comps)


def mesh_primitives(js, blob):
    out = []
    for mi, mesh in enumerate(js["meshes"]):
        extras = mesh.get("extras", {})
        if isinstance(extras, str):
            extras = json.loads(extras) if extras else {}
        names = extras.get("targetNames", [])
        for pi, prim in enumerate(mesh["primitives"]):
            attrs = prim.get("attributes", {})
            if "POSITION" not in attrs or "JOINTS_0" not in attrs:
                continue
            pos = read_acc(js, blob, attrs["POSITION"]).astype(np.float32)
            targets = {}
            for ti, t in enumerate(prim.get("targets", [])):
                nm = names[ti] if ti < len(names) else f"t{ti}"
                targets[nm] = read_acc(js, blob, t["POSITION"]).astype(np.float32)
            out.append((pos, targets))
    return out


def morphed(pos, targets, weights):
    out = pos.copy()
    for name, w in weights.items():
        if w and name in targets:
            out += targets[name] * w
    return out


def find_seam_pairs(prims):
    pairs = []
    positions = [p[0] for p in prims]
    for i in range(len(prims)):
        step = max(1, len(positions[i]) // 2500)
        for vi in range(0, len(positions[i]), step):
            for j in range(i + 1, len(prims)):
                d = np.linalg.norm(positions[j] - positions[i][vi], axis=1)
                jj = int(np.argmin(d))
                if d[jj] <= SEAM_EPS:
                    pairs.append((i, vi, j, jj))
    return pairs


def max_seam_gap(prims, pairs, weights):
    mx = 0.0
    for i, vi, j, jj in pairs:
        pi = morphed(prims[i][0], prims[i][1], weights)
        pj = morphed(prims[j][0], prims[j][1], weights)
        mx = max(mx, float(np.linalg.norm(pi[vi] - pj[jj])))
    return mx


def audit(path: Path) -> bool:
    js, blob = load_glb(path)
    prims = mesh_primitives(js, blob)
    pairs = find_seam_pairs(prims)
    neutral = max_seam_gap(prims, pairs, {})
    ok = True
    print(path.name, f"pairs={len(pairs)} neutral={neutral*1000:.2f}mm")
    poses = {
        "shoulders_1": {"shoulders_broad": 1.0},
        "torso_1": {"torso_broad": 1.0},
        "arms_1": {"arms_thick": 1.0},
        "hips_1": {"hips_broad": 1.0},
        "legs_1": {"legs_thick": 1.0},
        "combo": {"shoulders_broad": 1.0, "torso_broad": 1.0, "muscle_define": 1.0},
    }
    for name, weights in poses.items():
        gap = max_seam_gap(prims, pairs, weights)
        delta = gap - neutral
        print(f"  {name:12s} gap={gap*1000:5.2f}mm delta={delta*1000:5.2f}mm")
        if delta > MAX_SEAM_GAP_INCREASE_M:
            ok = False
    return ok


def main() -> int:
    ok = True
    for key in sys.argv[1:] or ["human_male", "human_female"]:
        path = REPO / "assets" / "units" / f"{key}.glb"
        ok &= audit(path)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
