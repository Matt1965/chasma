"""Find skinned vertices that move far from rest under a clip (all mesh nodes)."""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from glb_sanitize_animations import load_glb


def read_f32_vec(blob, gltf, acc_i, components):
    acc = gltf["accessors"][acc_i]
    bv = gltf["bufferViews"][acc["bufferView"]]
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = bv.get("byteStride", 4 * components) or (4 * components)
    fmt = "<" + "f" * components
    return [struct.unpack_from(fmt, blob, off + i * stride) for i in range(acc["count"])]


def sample_channel(gltf, blob, anim, node_i, path, t=0.0):
    for ch in anim.get("channels", []):
        tgt = ch["target"]
        if tgt.get("node") != node_i or tgt.get("path") != path:
            continue
        s = anim["samplers"][ch["sampler"]]
        times = [v[0] for v in read_f32_vec(blob, gltf, s["input"], 1)]
        comp = 3 if path != "rotation" else 4
        vals = read_f32_vec(blob, gltf, s["output"], comp)
        k = min(range(len(times)), key=lambda j: abs(times[j] - t))
        return vals[k]
    n = gltf["nodes"][node_i]
    if path == "translation":
        return tuple(n.get("translation", [0, 0, 0]))
    if path == "rotation":
        return tuple(n.get("rotation", [0, 0, 0, 1]))
    return tuple(n.get("scale", [1, 1, 1]))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("glb", type=Path)
    ap.add_argument("--clip", default="IdleBreathe")
    ap.add_argument("--threshold", type=float, default=2.0, help="max vertex displacement from rest (m)")
    args = ap.parse_args()
    gltf, blob = load_glb(args.glb)
    anim = next((a for a in gltf["animations"] if a.get("name") == args.clip), None)
    if not anim:
        raise SystemExit(f"no clip {args.clip}")

    pm = {}
    for i, n in enumerate(gltf["nodes"]):
        for c in n.get("children", []):
            pm[c] = i

    worst = []
    for ni, n in enumerate(gltf["nodes"]):
        if n.get("mesh") is None or n.get("skin") is None:
            continue
        skin = gltf["skins"][n["skin"]]
        # rest positions only: compare bind vs animated global translation of weighted joints (cheap proxy)
        prim = gltf["meshes"][n["mesh"]]["primitives"][0]
        pos = read_f32_vec(blob, gltf, prim["attributes"]["POSITION"], 3)
        rest_span = max(max(p[i] for p in pos) - min(p[i] for p in pos) for i in range(3))
        for ji, joint_node in enumerate(skin["joints"]):
            t0 = sample_channel(gltf, blob, anim, joint_node, "translation", 0.0)
            t1 = sample_channel(gltf, blob, anim, joint_node, "translation", 0.5)
            d = max(abs(t1[i] - t0[i]) for i in range(3))
            if d > 0.5:
                worst.append((d, gltf["nodes"][joint_node].get("name"), n.get("name"), rest_span))
    worst.sort(reverse=True)
    print(f"{args.glb.name} clip={args.clip} large joint translations (proxy):")
    for row in worst[:12]:
        print(f"  {row[1]} on {row[2]}: {row[0]:.2f}m (mesh rest span {row[3]:.2f}m)")


if __name__ == "__main__":
    main()
