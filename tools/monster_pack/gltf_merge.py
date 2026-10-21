"""glTF-level merge of FBX2glTF clip GLBs into the FBX2glTF SK GLB.

Channels are copied verbatim by node name. Keys are never re-anchored, clamped, or stripped;
if a clip disagrees with the SK bind pose that is a source defect to report, not to hide.
"""

from __future__ import annotations

import struct
from pathlib import Path
from typing import Any

from glb_sanitize_animations import load_glb, write_glb

EPS_T = 1e-5
EPS_R = 1e-5


class MergeError(Exception):
    pass


def read_acc(gltf, blob, i):
    a = gltf["accessors"][i]
    bv = gltf["bufferViews"][a["bufferView"]]
    off = bv.get("byteOffset", 0) + a.get("byteOffset", 0)
    comp = {"SCALAR": 1, "VEC3": 3, "VEC4": 4}[a["type"]]
    if a["componentType"] != 5126:
        raise MergeError(f"accessor {i}: unsupported component type (float only)")
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


def node_name_map(gltf: dict, *, strict: bool = True) -> dict[str, int]:
    out: dict[str, int] = {}
    dups: list[str] = []
    for i, n in enumerate(gltf["nodes"]):
        name = n.get("name")
        if not name:
            continue
        if name in out:
            dups.append(name)
            if not strict:
                continue
        out[name] = i
    if strict and dups:
        raise MergeError(f"duplicate node names in SK glTF: {sorted(set(dups))}")
    return out


def parent_map(gltf: dict) -> dict[int, int]:
    pm: dict[int, int] = {}
    for i, n in enumerate(gltf["nodes"]):
        for c in n.get("children", []):
            pm[c] = i
    return pm


def skin_joints(gltf: dict) -> set[int]:
    joints: set[int] = set()
    for s in gltf.get("skins", []):
        joints.update(s["joints"])
    return joints


def audit_sk(sk_gltf: dict) -> dict[str, Any]:
    names = node_name_map(sk_gltf)
    joints = skin_joints(sk_gltf)
    if not joints:
        raise MergeError("SK GLB has no skin joints")
    return {"node_count": len(sk_gltf["nodes"]), "named_nodes": len(names), "skin_joints": len(joints)}


def audit_clip_targets(sk_gltf: dict, clip_gltf: dict, clip_label: str) -> dict[str, Any]:
    sk_names = node_name_map(sk_gltf, strict=True)
    clip_names = node_name_map(clip_gltf, strict=False)
    sk_pm = parent_map(sk_gltf)
    clip_pm = parent_map(clip_gltf)
    parent_mismatch: list[str] = []
    anim_nodes = set()
    for anim in clip_gltf.get("animations", []):
        for ch in anim.get("channels", []):
            ni = ch["target"].get("node")
            if ni is not None:
                anim_nodes.add(clip_gltf["nodes"][ni].get("name"))
    missing = sorted(n for n in anim_nodes if n and n not in sk_names)
    for name in missing:
        pass
    for name in anim_nodes:
        if not name or name not in sk_names:
            continue
        ci = clip_names[name]
        si = sk_names[name]
        cp = clip_pm.get(ci)
        sp = sk_pm.get(si)
        c_parent = clip_gltf["nodes"][cp].get("name") if cp is not None else None
        s_parent = sk_gltf["nodes"][sp].get("name") if sp is not None else None
        if c_parent != s_parent:
            parent_mismatch.append(f"{name}: clip_parent={c_parent} sk_parent={s_parent}")
    return {
        "clip": clip_label,
        "clip_named_nodes": len(clip_names),
        "parent_mismatches": parent_mismatch,
        "animated_node_names": sorted(n for n in anim_nodes if n),
        "animated_nodes_missing_from_sk": missing,
    }


def defaults(node, path):
    if path == "translation":
        return tuple(node.get("translation", [0, 0, 0]))
    if path == "rotation":
        return tuple(node.get("rotation", [0, 0, 0, 1]))
    return tuple(node.get("scale", [1, 1, 1]))


def differs(a, b, eps):
    return max(abs(x - y) for x, y in zip(a, b)) > eps


def merge_clips_into_sk(
    sk_path: Path,
    clips: dict[str, Path],
    out: Path,
    *,
    fill_unkeyed: bool = True,
) -> dict[str, Any]:
    sk_gltf, sk_blob = load_glb(sk_path)
    sk_audit = audit_sk(sk_gltf)
    gltf, blob = load_glb(sk_path)
    blob = bytearray(blob)
    gltf["animations"] = []
    sk_names = node_name_map(gltf)
    joint_set = skin_joints(gltf)
    clip_reports: dict[str, Any] = {}
    target_audits: list[dict] = []

    for clip_name, clip_path in clips.items():
        cg, cb = load_glb(clip_path)
        target_audits.append(audit_clip_targets(sk_gltf, cg, clip_name))
        cn = node_name_map(cg, strict=False)
        if not cg.get("animations"):
            raise MergeError(f"clip `{clip_name}` has no animations")
        anim_src = cg["animations"][0]
        anim = {"name": clip_name, "channels": [], "samplers": []}
        keyed: set[tuple[str, str]] = set()
        t_min, t_max = float("inf"), float("-inf")
        copied = 0
        unmatched: list[str] = []
        interpolations: set[str] = set()
        for ch in anim_src["channels"]:
            tgt = ch["target"]
            nname = cg["nodes"][tgt["node"]].get("name")
            if nname not in sk_names:
                unmatched.append(nname or f"#{tgt['node']}")
                continue
            s = anim_src["samplers"][ch["sampler"]]
            interp = s.get("interpolation", "LINEAR")
            interpolations.add(interp)
            times, _ = read_acc(cg, cb, s["input"])
            vals, typ = read_acc(cg, cb, s["output"])
            t_min = min(t_min, times[0][0])
            t_max = max(t_max, times[-1][0])
            ia = append_acc(gltf, blob, times, "SCALAR")
            oa = append_acc(gltf, blob, vals, typ)
            anim["samplers"].append({"input": ia, "output": oa, "interpolation": interp})
            anim["channels"].append(
                {"sampler": len(anim["samplers"]) - 1, "target": {"node": sk_names[nname], "path": tgt["path"]}}
            )
            keyed.add((nname, tgt["path"]))
            copied += 1
        if copied == 0:
            raise MergeError(f"clip `{clip_name}` copied zero channels (all nodes unmatched)")
        skipped = sorted(set(unmatched))
        filled: list[str] = []
        if fill_unkeyed:
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
                        anim["channels"].append(
                            {"sampler": len(anim["samplers"]) - 1, "target": {"node": sk_names[nname], "path": path}}
                        )
                        filled.append(f"{nname}:{path}")
        gltf["animations"].append(anim)
        clip_reports[clip_name] = {
            "source_glb": str(clip_path),
            "copied_channels": copied,
            "skipped_channels_nodes_not_in_sk": skipped,
            "filled_constant_channels": filled,
            "time_range": [t_min, t_max],
            "interpolations": sorted(interpolations),
        }

    gltf["buffers"][0]["byteLength"] = len(blob)
    write_glb(out, gltf, bytes(blob))
    return {"sk_audit": sk_audit, "clip_target_audits": target_audits, "clips": clip_reports}
