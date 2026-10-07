#!/usr/bin/env python3
"""Fail if linear-blend skinning at idle t=0 blows up vertex positions."""

from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path

from glb_sanitize_animations import load_glb

_IDENTITY = (0.0, 0.0, 0.0, 1.0)


def _quat_mul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    )


def _mat4_identity():
    return [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]


def _mat4_mul(a, b):
    out = [[0.0] * 4 for _ in range(4)]
    for c in range(4):
        for r in range(4):
            out[r][c] = sum(a[r][k] * b[k][c] for k in range(4))
    return out


def _mat3_from_quat(q):
    x, y, z, w = q
    xx, yy, zz = x * x, y * y, z * z
    xy, xz, yz = x * y, x * z, y * z
    wx, wy, wz = w * x, w * y, w * z
    return [
        [1.0 - 2.0 * (yy + zz), 2.0 * (xy + wz), 2.0 * (xz - wy)],
        [2.0 * (xy - wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz + wx)],
        [2.0 * (xz + wy), 2.0 * (yz - wx), 1.0 - 2.0 * (xx + yy)],
    ]


def _mat4_from_trs(t, r, s):
    rot = _mat3_from_quat(r)
    sx, sy, sz = s
    m = _mat4_identity()
    for row in range(3):
        for col in range(3):
            m[row][col] = rot[row][col] * (sx if col == 0 else sy if col == 1 else sz)
    m[0][3], m[1][3], m[2][3] = t[0], t[1], t[2]
    return m


def _parents(gltf):
    p = {}
    for pi, n in enumerate(gltf["nodes"]):
        for c in n.get("children", []):
            p[int(c)] = pi
    return p


def _global_mat(idx, gltf, parents, cache):
    if idx in cache:
        return cache[idx]
    node = gltf["nodes"][idx]
    t = node.get("translation", [0.0, 0.0, 0.0])
    r = tuple(node.get("rotation", _IDENTITY))
    s = node.get("scale", [1.0, 1.0, 1.0])
    local = _mat4_from_trs(t, r, s)
    if idx not in parents:
        cache[idx] = local
        return local
    g = _mat4_mul(_global_mat(parents[idx], gltf, parents, cache), local)
    cache[idx] = g
    return g


def _read_accessor_f32(blob, gltf, accessor_index):
    acc = gltf["accessors"][accessor_index]
    bv = gltf["bufferViews"][acc["bufferView"]]
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    count = acc["count"]
    ctype = acc["componentType"]
    atype = acc["type"]
    comps = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[atype]
    stride = bv.get("byteStride", comps * 4)
    out = []
    for i in range(count):
        base = off + i * stride
        if atype == "MAT4":
            row = []
            for j in range(16):
                row.append(struct.unpack_from("<f", blob, base + j * 4)[0])
            out.append(row)
        else:
            row = []
            for j in range(comps):
                row.append(struct.unpack_from("<f", blob, base + j * 4)[0])
            out.append(tuple(row) if comps > 1 else row[0])
    return out


def _sample_rotation_channel(gltf, blob, anim, node_index):
    for ch in anim.get("channels", []):
        tgt = ch["target"]
        if tgt.get("node") != node_index or tgt.get("path") != "rotation":
            continue
        samp = anim["samplers"][ch["sampler"]]
        outs = _read_accessor_f32(blob, gltf, samp["output"])
        return outs[0]
    node = gltf["nodes"][node_index]
    return tuple(node.get("rotation", _IDENTITY))


def _joint_matrices(gltf, blob, skin, rot_overrides):
    parents = _parents(gltf)
    cache: dict[int, list[list[float]]] = {}
    ibms = _read_accessor_f32(blob, gltf, skin["inverseBindMatrices"])
    mats = []
    for ji, node_index in enumerate(skin["joints"]):
        node = gltf["nodes"][node_index]
        t = node.get("translation", [0.0, 0.0, 0.0])
        r = rot_overrides.get(node_index, tuple(node.get("rotation", _IDENTITY)))
        s = node.get("scale", [1.0, 1.0, 1.0])
        local = _mat4_from_trs(t, r, s)
        if node_index in parents:
            g = _mat4_mul(_global_mat(parents[node_index], gltf, parents, cache), local)
        else:
            g = local
        ibm = ibms[ji]
        ibm4 = [[ibm[c * 4 + r] for c in range(4)] for r in range(4)]
        mats.append(_mat4_mul(g, ibm4))
    return mats


def _transform_point(m, p):
    x, y, z = p
    return (
        m[0][0] * x + m[0][1] * y + m[0][2] * z + m[0][3],
        m[1][0] * x + m[1][1] * y + m[1][2] * z + m[1][3],
        m[2][0] * x + m[2][1] * y + m[2][2] * z + m[2][3],
    )


def check(path: Path, clip_name: str = "IdleBreathe", max_span: float = 25.0) -> int:
    gltf, blob = load_glb(path)
    skin = gltf["skins"][0]
    anim = next((a for a in gltf["animations"] if a.get("name") == clip_name), None)
    if anim is None:
        print(f"FAIL {path.name}: missing clip {clip_name}")
        return 1

    rot_overrides = {}
    for node_index in skin["joints"]:
        rot_overrides[node_index] = _sample_rotation_channel(gltf, blob, anim, node_index)

    joint_mats = _joint_matrices(gltf, blob, skin, rot_overrides)
    prim = gltf["meshes"][0]["primitives"][0]
    positions = _read_accessor_f32(blob, gltf, prim["attributes"]["POSITION"])
    joints = _read_accessor_f32(blob, gltf, prim["attributes"]["JOINTS_0"])
    weights = _read_accessor_f32(blob, gltf, prim["attributes"]["WEIGHTS_0"])

    xs, ys, zs = [], [], []
    for pos, j4, w4 in zip(positions, joints, weights):
        wx = wy = wz = 0.0
        tw = 0.0
        for j, w in zip(j4, w4):
            if w <= 0:
                continue
            ji = int(j)
            tx, ty, tz = _transform_point(joint_mats[ji], pos)
            wx += w * tx
            wy += w * ty
            wz += w * tz
            tw += w
        if tw > 0:
            wx /= tw
            wy /= tw
            wz /= tw
        xs.append(wx)
        ys.append(wy)
        zs.append(wz)

    span = max(max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    if span > max_span:
        print(f"FAIL {path.name}: skinned idle span {span:.1f}m (max {max_span})")
        return 1
    print(f"OK {path.name}: skinned idle span {span:.2f}m")
    return 0


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("glb", type=Path)
    parser.add_argument("--clip", default="IdleBreathe")
    parser.add_argument("--max-span", type=float, default=25.0)
    args = parser.parse_args()
    sys.exit(check(args.glb.resolve(), args.clip, args.max_span))


if __name__ == "__main__":
    main()
