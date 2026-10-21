#!/usr/bin/env python3
"""Sample root/body/jaw motion in bufomorph GLB clips (diagnostic)."""

from __future__ import annotations

import json
import struct
import sys
from pathlib import Path

_SCRIPT = Path(__file__).resolve().parent
if str(_SCRIPT) not in sys.path:
    sys.path.insert(0, str(_SCRIPT))

from glb_sanitize_animations import load_glb

BONES = ("root", "Bufomorph_", "Bufomorph_JawUpper", "Tongue1")


def quat_rotate(q: list[float], v: list[float]) -> list[float]:
    x, y, z, w = q
    vx, vy, vz = v
    cx = y * vz - z * vy
    cy = z * vx - x * vz
    cz = x * vy - y * vx
    return [
        vx + 2 * (w * cx + y * cz - z * cy),
        vy + 2 * (w * cy + z * cx - x * cz),
        vz + 2 * (w * cz + x * cy - y * cx),
    ]


def read_times(blob: bytes, gltf: dict, accessor_index: int) -> list[float]:
    acc = gltf["accessors"][accessor_index]
    bv = gltf["bufferViews"][acc["bufferView"]]
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = 4
    return [
        struct.unpack_from("<f", blob, off + i * stride)[0] for i in range(acc["count"])
    ]


def read_vec3_keys(blob: bytes, gltf: dict, accessor_index: int) -> list[tuple[float, float, float]]:
    acc = gltf["accessors"][accessor_index]
    bv = gltf["bufferViews"][acc["bufferView"]]
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    out = []
    for i in range(acc["count"]):
        x, y, z = struct.unpack_from("<fff", blob, off + i * 12)
        out.append((x, y, z))
    return out


def read_quat_keys(blob: bytes, gltf: dict, accessor_index: int) -> list[tuple[float, float, float, float]]:
    acc = gltf["accessors"][accessor_index]
    bv = gltf["bufferViews"][acc["bufferView"]]
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    out = []
    for i in range(acc["count"]):
        x, y, z, w = struct.unpack_from("<ffff", blob, off + i * 16)
        out.append((x, y, z, w))
    return out


def bone_index(gltf: dict, name: str) -> int | None:
    for i, n in enumerate(gltf.get("nodes", [])):
        if (n.get("name") or "").strip() == name:
            return i
    return None


def sample_channel(
    gltf: dict, blob: bytes, anim_name: str, bone: str, path: str, time_s: float
) -> dict | None:
    bi = bone_index(gltf, bone)
    if bi is None:
        return None
    anim = next((a for a in gltf["animations"] if a.get("name") == anim_name), None)
    if anim is None:
        return {"error": f"missing clip {anim_name}"}
    for ch in anim.get("channels", []):
        tgt = ch.get("target", {})
        if tgt.get("node") != bi or tgt.get("path") != path:
            continue
        sampler = anim["samplers"][ch["sampler"]]
        times = read_times(blob, gltf, sampler["input"])
        outputs = sampler["output"]
        if path == "translation":
            vals = read_vec3_keys(blob, gltf, outputs)
        elif path == "rotation":
            vals = read_quat_keys(blob, gltf, outputs)
        else:
            vals = []
        # nearest key
        idx = min(range(len(times)), key=lambda i: abs(times[i] - time_s))
        return {
            "clip": anim_name,
            "bone": bone,
            "path": path,
            "t_key": times[idx],
            "value": vals[idx] if idx < len(vals) else None,
        }
    return {"clip": anim_name, "bone": bone, "path": path, "missing_channel": True}


def scene_rotation(gltf: dict) -> list[float]:
    scene_i = gltf["scenes"][0]["nodes"][0]
    return list(gltf["nodes"][scene_i].get("rotation", [0, 0, 0, 1]))


def main() -> None:
    path = Path(__file__).resolve().parents[2] / "assets/units/bufomorph.glb"
    gltf, blob = load_glb(path)
    report = {
        "glb": str(path),
        "scene_rotation": scene_rotation(gltf),
        "clips": {},
    }
    for clip in ("IdleLookAround", "IdleBreathe", "BiteAttack", "Run"):
        report["clips"][clip] = []
        for bone in BONES:
            for p in ("translation", "rotation"):
                s = sample_channel(gltf, blob, clip, bone, p, 0.0)
                if s:
                    report["clips"][clip].append(s)
        # bite mid-clip
        if clip == "BiteAttack":
            for bone in ("root", "Bufomorph_"):
                for p in ("translation", "rotation"):
                    s = sample_channel(gltf, blob, clip, bone, p, 0.35)
                    if s:
                        s["sample_time"] = 0.35
                        report["clips"][clip].append(s)
    # jaw up vector at idle t=0 for look vs breathe
    for clip in ("IdleLookAround", "IdleBreathe"):
        jaw = sample_channel(gltf, blob, clip, "Bufomorph_JawUpper", "rotation", 0.0)
        if jaw and jaw.get("value"):
            q = list(jaw["value"])
            up = quat_rotate(q, [0.0, 1.0, 0.0])
            report[f"{clip}_jaw_local_up"] = up
    out = Path(__file__).resolve().parent / "diagnostics/bufomorph/CLIP_MOTION_SAMPLES.json"
    out.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(out)


if __name__ == "__main__":
    main()
