"""PROTOTYPE (not the production export path): glTF-level clip merge.

Merges FBX2glTF clip GLBs into the FBX2glTF SK GLB *at the glTF level* -- no Blender in the
skeleton/animation path. Validated on Bufomorph (see diagnostics/bufomorph/INDEPENDENT_REVIEW.md)
as the candidate replacement for `monster_export_core.merge_action` + Blender glTF export.

What it does
  * Copies every animation channel from each clip GLB onto the SK node with the same name,
    keeping the clip's own sampler data (meters, absolute TRS). No unit or reference-space
    transforms are applied, because FBX2glTF already emits both files in the same space.
  * For skin joints a clip does NOT key, writes a constant STEP channel holding the clip file's
    node default when it differs from the SK bind value. FBX2glTF drops channels that are
    constant and equal to the node default *of the clip file*; an absolute-TRS player (Unity)
    poses those joints from the clip rig, so this is required to reproduce the authored pose
    (Bufomorph jaw/tongue closed in idle).

What it does not do
  * Materials / textures: FBX2glTF SK output carries the FBX material slots only. Use the
    glTF-level material step (see review) or the existing Blender texture bind.
  * Root motion policy: `_RM` clips are copied as-is; runtime decides what to do with `root`.

Usage
  py -3 merge_fbx2gltf_clips.py --sk SK_X.glb --out X.glb --clip Idle=X@Idle.glb --clip Run=X@Run.glb ...
  (print a JSON report of copied / filled channels per clip)
"""
from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from glb_sanitize_animations import load_glb, write_glb  # noqa: E402

EPS_T = 1e-5
EPS_R = 1e-5


def read_acc(gltf, blob, i):
    a = gltf["accessors"][i]
    bv = gltf["bufferViews"][a["bufferView"]]
    off = bv.get("byteOffset", 0) + a.get("byteOffset", 0)
    comp = {"SCALAR": 1, "VEC3": 3, "VEC4": 4}[a["type"]]
    assert a["componentType"] == 5126, "prototype supports float accessors only"
    size = 4 * comp
    stride = bv.get("byteStride", size)
    return [struct.unpack_from("<" + "f" * comp, blob, off + k * stride) for k in range(a["count"])], a["type"]


def append_acc(gltf, blob: bytearray, values, typ):
    comp = {"SCALAR": 1, "VEC3": 3, "VEC4": 4}[typ]
    while len(blob) % 4:
        blob.append(0)
    off = len(blob)
    for v in values:
        blob.extend(struct.pack("<" + "f" * comp, *v))
    gltf["bufferViews"].append({"buffer": 0, "byteOffset": off, "byteLength": len(blob) - off})
    acc = {"bufferView": len(gltf["bufferViews"]) - 1, "componentType": 5126, "count": len(values), "type": typ}
    if typ == "SCALAR":
        flat = [v[0] for v in values]
        acc["min"] = [min(flat)]
        acc["max"] = [max(flat)]
    gltf["accessors"].append(acc)
    return len(gltf["accessors"]) - 1


def names(gltf):
    return {n.get("name"): i for i, n in enumerate(gltf["nodes"])}


def defaults(node, path):
    if path == "translation":
        return tuple(node.get("translation", [0, 0, 0]))
    if path == "rotation":
        return tuple(node.get("rotation", [0, 0, 0, 1]))
    return tuple(node.get("scale", [1, 1, 1]))


def differs(a, b, eps):
    return max(abs(x - y) for x, y in zip(a, b)) > eps


def merge(sk_path: Path, clips: dict[str, Path], out: Path, fill_unkeyed: bool = True):
    gltf, blob = load_glb(sk_path)
    blob = bytearray(blob)
    gltf["animations"] = []
    sk_names = names(gltf)
    joint_set = set()
    for s in gltf.get("skins", []):
        joint_set.update(s["joints"])
    report = {}
    for clip_name, clip_path in clips.items():
        cg, cb = load_glb(clip_path)
        cn = names(cg)
        if not cg.get("animations"):
            report[clip_name] = {"error": "clip GLB has no animations"}
            continue
        anim_src = cg["animations"][0]
        anim = {"name": clip_name, "channels": [], "samplers": []}
        keyed = set()
        t_min, t_max = float("inf"), float("-inf")
        copied = 0
        unmatched = []
        for ch in anim_src["channels"]:
            tgt = ch["target"]
            nname = cg["nodes"][tgt["node"]].get("name")
            if nname not in sk_names:
                unmatched.append(nname)
                continue
            s = anim_src["samplers"][ch["sampler"]]
            times, _ = read_acc(cg, cb, s["input"])
            vals, typ = read_acc(cg, cb, s["output"])
            t_min = min(t_min, times[0][0])
            t_max = max(t_max, times[-1][0])
            ia = append_acc(gltf, blob, times, "SCALAR")
            oa = append_acc(gltf, blob, vals, typ)
            anim["samplers"].append({"input": ia, "output": oa, "interpolation": s.get("interpolation", "LINEAR")})
            anim["channels"].append({"sampler": len(anim["samplers"]) - 1, "target": {"node": sk_names[nname], "path": tgt["path"]}})
            keyed.add((nname, tgt["path"]))
            copied += 1
        filled = []
        if fill_unkeyed and copied:
            for nname, ci in cn.items():
                if nname not in sk_names or sk_names[nname] not in joint_set:
                    continue
                for path, eps, typ in (("translation", EPS_T, "VEC3"), ("rotation", EPS_R, "VEC4")):
                    if (nname, path) in keyed:
                        continue
                    cd = defaults(cg["nodes"][ci], path)
                    sd = defaults(gltf["nodes"][sk_names[nname]], path)
                    if differs(cd, sd, eps):
                        ia = append_acc(gltf, blob, [(t_min,), (t_max,)], "SCALAR")
                        oa = append_acc(gltf, blob, [cd, cd], typ)
                        anim["samplers"].append({"input": ia, "output": oa, "interpolation": "STEP"})
                        anim["channels"].append({"sampler": len(anim["samplers"]) - 1, "target": {"node": sk_names[nname], "path": path}})
                        filled.append(f"{nname}:{path}")
        gltf["animations"].append(anim)
        report[clip_name] = {
            "copied_channels": copied,
            "unmatched_clip_nodes": sorted(set(unmatched)),
            "filled_constant_channels": filled,
            "time_range": [t_min, t_max],
        }
    gltf["buffers"][0]["byteLength"] = len(blob)
    write_glb(out, gltf, bytes(blob))
    return report


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--sk", required=True, type=Path, help="FBX2glTF output for the SK_*.fbx")
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--clip", action="append", default=[], metavar="NAME=PATH", help="FBX2glTF output for one @Clip.fbx")
    ap.add_argument("--no-fill", action="store_true", help="do not synthesize constant channels for un-keyed joints")
    args = ap.parse_args(argv)
    clips = {}
    for spec in args.clip:
        name, _, path = spec.partition("=")
        clips[name] = Path(path)
    rep = merge(args.sk, clips, args.out, fill_unkeyed=not args.no_fill)
    print(json.dumps(rep, indent=1))


if __name__ == "__main__":
    main()
