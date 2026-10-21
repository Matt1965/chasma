"""Diagnostic: flag animated joint translations that are impossibly large for the rig.

For each GLB: compute the rest-skeleton extent (max axis span of all skin-joint world
positions), then the largest peak-to-peak translation excursion of any joint in any clip.
A joint that moves further than the whole skeleton is wide cannot be authored in-place
motion; on the monster pack it is the signature of the Blender pose-delta merge applying
centimetre deltas to a metre rig (100x). Root-motion (`_RM`) clips can be excluded.

Usage:  py -3 audit_joint_translation_scale.py [--no-rm] [--ratio 1.0] assets/units/*.glb
Exit code 1 if any file exceeds the ratio.
"""
from __future__ import annotations

import argparse
import math
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from glb_sanitize_animations import load_glb  # noqa: E402


def acc(gltf, blob, i, fmt):
    a = gltf["accessors"][i]
    bv = gltf["bufferViews"][a["bufferView"]]
    off = bv.get("byteOffset", 0) + a.get("byteOffset", 0)
    size = struct.calcsize(fmt)
    stride = bv.get("byteStride", size)
    return [struct.unpack_from(fmt, blob, off + k * stride) for k in range(a["count"])]


def qmul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz)


def qrot(q, v):
    x, y, z, w = q
    vx, vy, vz = v
    cx = y * vz - z * vy
    cy = z * vx - x * vz
    cz = x * vy - y * vx
    return (vx + 2 * (w * cx + y * cz - z * cy), vy + 2 * (w * cy + z * cx - x * cz), vz + 2 * (w * cz + x * cy - y * cx))


def rest_world_positions(gltf):
    parent = {}
    for i, n in enumerate(gltf["nodes"]):
        for c in n.get("children", []):
            parent[c] = i
    cache = {}

    def wm(i):
        if i in cache:
            return cache[i]
        n = gltf["nodes"][i]
        t = tuple(n.get("translation", [0, 0, 0]))
        r = tuple(n.get("rotation", [0, 0, 0, 1]))
        s = tuple(n.get("scale", [1, 1, 1]))
        if i in parent:
            pt, pr, ps = wm(parent[i])
            wt = tuple(pt[k] + c for k, c in enumerate(qrot(pr, tuple(t[k] * ps[k] for k in range(3)))))
            cache[i] = (wt, qmul(pr, r), tuple(ps[k] * s[k] for k in range(3)))
        else:
            cache[i] = (t, r, s)
        return cache[i]

    return {i: wm(i)[0] for i in range(len(gltf["nodes"]))}


def audit(path: Path, skip_rm: bool):
    gltf, blob = load_glb(path)
    anims = gltf.get("animations") or []
    if not anims:
        return dict(file=path.stem, status="no animations")
    joints = set()
    for s in gltf.get("skins", []):
        joints.update(s["joints"])
    if not joints:
        return dict(file=path.stem, status="no skin joints")
    wp = rest_world_positions(gltf)
    pts = [wp[j] for j in joints]
    ext = max(max(p[k] for p in pts) - min(p[k] for p in pts) for k in range(3))
    worst = (0.0, None, None)
    for a in anims:
        name = a.get("name") or ""
        if skip_rm and name.endswith("_RM"):
            continue
        for ch in a["channels"]:
            tgt = ch["target"]
            if tgt["path"] != "translation" or tgt.get("node") not in joints:
                continue
            vals = acc(gltf, blob, a["samplers"][ch["sampler"]]["output"], "<fff")
            pp = max(max(v[k] for v in vals) - min(v[k] for v in vals) for k in range(3))
            if pp > worst[0]:
                worst = (pp, name, gltf["nodes"][tgt["node"]].get("name"))
    ratio = worst[0] / ext if ext else math.inf
    return dict(file=path.stem, status="ok", extent_m=ext, worst_pp_m=worst[0], ratio=ratio, clip=worst[1], joint=worst[2], anims=len(anims))


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("files", nargs="+", type=Path)
    ap.add_argument("--no-rm", action="store_true", help="ignore *_RM root-motion clips")
    ap.add_argument("--ratio", type=float, default=1.0, help="flag threshold: worst excursion / skeleton extent")
    args = ap.parse_args(argv)
    failed = 0
    for f in args.files:
        r = audit(f, args.no_rm)
        if r["status"] != "ok":
            print(f"{r['file']:16s} {r['status']}")
            continue
        flag = "RED" if r["ratio"] > args.ratio else "ok"
        failed += flag == "RED"
        print(f"{r['file']:16s} {flag:3s} extent={r['extent_m']:.2f}m worst_pp={r['worst_pp_m']:.2f}m ratio={r['ratio']:.1f} clip={r['clip']} joint={r['joint']} anims={r['anims']}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
