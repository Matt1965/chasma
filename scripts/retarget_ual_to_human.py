#!/usr/bin/env python3
"""Retarget UAL animations onto Chasma Human skeleton bind pose (Slice 8).

Reads UAL1_Standard.glb (non-root-motion) and rebases selected clips against the
current Human unit GLB skeleton rest pose. Preserves the existing Mine clip.

Output writes retargeted animation data onto each human body's own mesh/skeleton:
  - human_male.glb  <- existing male unit GLB (mesh + Mine clip)
  - human_female.glb <- superhero_female_animated.glb base (distinct female mesh)
"""

from __future__ import annotations

import json
import shutil
import struct
from dataclasses import dataclass
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[1]
UAL_GLB = (
    REPO
    / "_tmp_ual_source"
    / "Universal Animation Library[Standard]"
    / "Unreal-Godot"
    / "UAL1_Standard.glb"
)
HUMAN_MALE = REPO / "assets" / "units" / "human_male.glb"
HUMAN_FEMALE = REPO / "assets" / "units" / "human_female.glb"
HUMAN_FEMALE_SOURCE = (
    REPO
    / "_tmp_human_assets"
    / "chasma_animated_characters"
    / "superhero_female_animated.glb"
)
LICENSE_DST = REPO / "assets" / "animations" / "licenses" / "UAL1_Standard_CC0.txt"

# UAL clip name -> Chasma runtime clip name
CLIP_MAP: dict[str, str] = {
    "Idle_Loop": "Idle",
    "Walk_Loop": "Walk",
    "Jog_Fwd_Loop": "Run",
    "Death01": "Death",
    "Hit_Chest": "Hit",
    "Punch_Jab": "Punch_Jab",
    "Punch_Cross": "Punch_Cross",
    "Sword_Attack": "Sword_Attack",
    "Sword_Idle": "Sword_Idle",
}

PRESERVE_CLIP = "Mine"


@dataclass
class Trs:
    t: np.ndarray
    r: np.ndarray  # xyzw
    s: np.ndarray


def quat_to_mat(q: np.ndarray) -> np.ndarray:
    x, y, z, w = q
    return np.array(
        [
            [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w), 0],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w), 0],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y), 0],
            [0, 0, 0, 1],
        ],
        dtype=np.float64,
    )


def mat_to_trs(m: np.ndarray) -> Trs:
    t = m[:3, 3].copy()
    sx = np.linalg.norm(m[:3, 0])
    sy = np.linalg.norm(m[:3, 1])
    sz = np.linalg.norm(m[:3, 2])
    s = np.array([sx, sy, sz], dtype=np.float64)
    rot = m[:3, :3] / np.maximum(s, 1e-8)
    trace = rot[0, 0] + rot[1, 1] + rot[2, 2]
    if trace > 0:
        s_root = np.sqrt(trace + 1.0) * 2
        w = 0.25 * s_root
        x = (rot[2, 1] - rot[1, 2]) / s_root
        y = (rot[0, 2] - rot[2, 0]) / s_root
        z = (rot[1, 0] - rot[0, 1]) / s_root
    elif rot[0, 0] > rot[1, 1] and rot[0, 0] > rot[2, 2]:
        s_root = np.sqrt(1.0 + rot[0, 0] - rot[1, 1] - rot[2, 2]) * 2
        w = (rot[2, 1] - rot[1, 2]) / s_root
        x = 0.25 * s_root
        y = (rot[0, 1] + rot[1, 0]) / s_root
        z = (rot[0, 2] + rot[2, 0]) / s_root
    elif rot[1, 1] > rot[2, 2]:
        s_root = np.sqrt(1.0 + rot[1, 1] - rot[0, 0] - rot[2, 2]) * 2
        w = (rot[0, 2] - rot[2, 0]) / s_root
        x = (rot[0, 1] + rot[1, 0]) / s_root
        y = 0.25 * s_root
        z = (rot[1, 2] + rot[2, 1]) / s_root
    else:
        s_root = np.sqrt(1.0 + rot[2, 2] - rot[0, 0] - rot[1, 1]) * 2
        w = (rot[1, 0] - rot[0, 1]) / s_root
        x = (rot[0, 2] + rot[2, 0]) / s_root
        y = (rot[1, 2] + rot[2, 1]) / s_root
        z = 0.25 * s_root
    r = np.array([x, y, z, w], dtype=np.float64)
    r /= np.linalg.norm(r)
    return Trs(t=t.astype(np.float32), r=r.astype(np.float32), s=s.astype(np.float32))


def trs_to_mat(trs: Trs) -> np.ndarray:
    m = quat_to_mat(trs.r)
    m[:3, 0] *= trs.s[0]
    m[:3, 1] *= trs.s[1]
    m[:3, 2] *= trs.s[2]
    m[:3, 3] = trs.t
    return m


def load_glb(path: Path) -> tuple[dict, bytearray]:
    data = path.read_bytes()
    off = 12
    cl, _ = struct.unpack_from("<II", data, off)
    off += 8
    js = json.loads(data[off : off + cl])
    off = 12
    bin_blob = bytearray()
    while off < len(data):
        cl, ty = struct.unpack_from("<II", data, off)
        off += 8
        chunk = data[off : off + cl]
        off += cl
        if ty == 0x004E4942:
            bin_blob = bytearray(chunk)
    return js, bin_blob


def save_glb(path: Path, js: dict, bin_blob: bytearray) -> None:
    json_bytes = json.dumps(js, separators=(",", ":")).encode("utf-8")
    json_pad = (4 - len(json_bytes) % 4) % 4
    json_bytes += b" " * json_pad
    bin_pad = (4 - len(bin_blob) % 4) % 4
    bin_blob.extend(b"\x00" * bin_pad)
    total = 12 + 8 + len(json_bytes) + 8 + len(bin_blob)
    out = bytearray()
    out.extend(struct.pack("<III", 0x46546C67, 2, total))
    out.extend(struct.pack("<II", len(json_bytes), 0x4E4F534A))
    out.extend(json_bytes)
    out.extend(struct.pack("<II", len(bin_blob), 0x004E4942))
    out.extend(bin_blob)
    path.write_bytes(out)


def node_bind_trs(js: dict, node_idx: int) -> Trs:
    n = js["nodes"][node_idx]
    t = np.array(n.get("translation", [0, 0, 0]), dtype=np.float64)
    r = np.array(n.get("rotation", [0, 0, 0, 1]), dtype=np.float64)
    s = np.array(n.get("scale", [1, 1, 1]), dtype=np.float64)
    return Trs(t=t.astype(np.float32), r=r.astype(np.float32), s=s.astype(np.float32))


def read_accessor(js: dict, blob: bytearray, accessor_idx: int) -> np.ndarray:
    acc = js["accessors"][accessor_idx]
    bv = js["bufferViews"][acc["bufferView"]]
    start = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    count = acc["count"]
    ctype = acc["componentType"]
    atype = acc["type"]
    comps = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[atype]
    dtype = {5126: np.float32, 5123: np.uint16, 5125: np.uint32}[ctype]
    stride = bv.get("byteStride", comps * np.dtype(dtype).itemsize)
    raw = blob[start : start + count * stride]
    arr = np.frombuffer(raw, dtype=dtype, count=count * comps)
    return arr.reshape(count, comps)


def append_bytes(blob: bytearray, data: bytes) -> int:
    off = len(blob)
    blob.extend(data)
    return off


def write_accessor(
    js: dict,
    blob: bytearray,
    values: np.ndarray,
    atype: str,
) -> int:
    values = np.asarray(values, dtype=np.float32)
    if atype == "SCALAR":
        flat = values.reshape(-1)
    else:
        flat = values.reshape(-1)
    raw = flat.tobytes()
    byte_offset = append_bytes(blob, raw)
    bv_idx = len(js["bufferViews"])
    js["bufferViews"].append(
        {
            "buffer": 0,
            "byteOffset": byte_offset,
            "byteLength": len(raw),
        }
    )
    acc_idx = len(js["accessors"])
    js["accessors"].append(
        {
            "bufferView": bv_idx,
            "componentType": 5126,
            "count": len(values) if atype != "SCALAR" else values.shape[0],
            "type": atype,
        }
    )
    return acc_idx


def retarget_trs(
    anim: Trs,
    bind_src: Trs,
    bind_tgt: Trs,
) -> Trs:
    m_anim = trs_to_mat(anim)
    m_bind_src = trs_to_mat(bind_src)
    m_bind_tgt = trs_to_mat(bind_tgt)
    m_out = m_bind_tgt @ np.linalg.inv(m_bind_src) @ m_anim
    return mat_to_trs(m_out)


def retarget_animation(
    src_js: dict,
    src_blob: bytearray,
    tgt_js: dict,
    anim_idx: int,
    joint_indices: list[int],
    bind_src: list[Trs],
    bind_tgt: list[Trs],
    out_blob: bytearray,
) -> dict:
    anim = src_js["animations"][anim_idx]
    node_channels: dict[int, dict[str, tuple[np.ndarray, np.ndarray]]] = {}
    for channel in anim["channels"]:
        target = channel["target"]
        node = target["node"]
        path = target["path"]
        sampler = anim["samplers"][channel["sampler"]]
        times = read_accessor(src_js, src_blob, sampler["input"]).reshape(-1)
        values = read_accessor(src_js, src_blob, sampler["output"])
        node_channels.setdefault(node, {})[path] = (times, values)

    pending: list[tuple[str, int, np.ndarray, np.ndarray]] = []
    for joint_order_idx, node in enumerate(joint_indices):
        ch = node_channels.get(node, {})
        bsrc = bind_src[joint_order_idx]
        btgt = bind_tgt[joint_order_idx]

        if "rotation" in ch:
            times, values = ch["rotation"]
            out_vals = []
            for q in values:
                anim_trs = Trs(t=btgt.t, r=q.astype(np.float32), s=btgt.s)
                out_vals.append(retarget_trs(anim_trs, bsrc, btgt).r)
            pending.append(
                ("rotation", node, times, np.array(out_vals, dtype=np.float32))
            )

        if "translation" in ch:
            times, values = ch["translation"]
            out_vals = []
            for t in values:
                anim_trs = Trs(t=t.astype(np.float32), r=bsrc.r, s=bsrc.s)
                out_vals.append(retarget_trs(anim_trs, bsrc, btgt).t)
            pending.append(
                ("translation", node, times, np.array(out_vals, dtype=np.float32))
            )

        # Scale channels are exporter noise for humanoid locomotion/combat clips.
        # Target skeleton retains canonical bind scale; animation drives rotation + translation.

    samplers = []
    channels = []
    for path, node, times, out_arr in pending:
        input_acc = write_accessor(tgt_js, out_blob, times, "SCALAR")
        atype = {"rotation": "VEC4", "translation": "VEC3", "scale": "VEC3"}[path]
        output_acc = write_accessor(tgt_js, out_blob, out_arr, atype)
        sampler_idx = len(samplers)
        samplers.append({"input": input_acc, "output": output_acc})
        channels.append({"sampler": sampler_idx, "target": {"node": node, "path": path}})

    return {"samplers": samplers, "channels": channels, "name": anim.get("name", "")}


def extract_animation_by_name(js: dict, name: str) -> dict | None:
    for anim in js.get("animations", []):
        if anim.get("name") == name:
            return anim
    return None


def build_retargeted_human(base_path: Path, output_path: Path) -> None:
    """Retarget UAL clips onto `base_path` skeleton/mesh and write `output_path`."""
    if not UAL_GLB.is_file():
        raise SystemExit(f"Missing UAL source: {UAL_GLB}")
    if not base_path.is_file():
        raise SystemExit(f"Missing human base GLB: {base_path}")

    src_js, src_blob = load_glb(UAL_GLB)
    base_js, base_blob = load_glb(base_path)
    joint_indices = src_js["skins"][0]["joints"]
    bind_src = [node_bind_trs(src_js, j) for j in joint_indices]
    bind_tgt = [node_bind_trs(base_js, j) for j in joint_indices]

    mine_anim = extract_animation_by_name(base_js, PRESERVE_CLIP)
    if mine_anim is None:
        raise SystemExit(f"{base_path} missing required `{PRESERVE_CLIP}` clip")

    out_js = json.loads(json.dumps(base_js))
    out_blob = bytearray(base_blob)
    out_js["animations"] = []

    mine_copy = json.loads(json.dumps(mine_anim))
    mine_copy["name"] = PRESERVE_CLIP
    out_js["animations"].append(mine_copy)

    src_name_to_idx = {
        a.get("name", ""): i for i, a in enumerate(src_js.get("animations", []))
    }

    for ual_name, chasma_name in CLIP_MAP.items():
        if ual_name not in src_name_to_idx:
            raise SystemExit(f"UAL missing clip {ual_name}")
        retargeted = retarget_animation(
            src_js,
            src_blob,
            out_js,
            src_name_to_idx[ual_name],
            joint_indices,
            bind_src,
            bind_tgt,
            out_blob,
        )
        retargeted["name"] = chasma_name
        out_js["animations"].append(retargeted)
        print(f"{output_path.name}: retargeted {ual_name} -> {chasma_name}")

    from author_human_morph_targets import author_morph_targets

    author_morph_targets(out_js, out_blob)
    out_js["buffers"] = [{"byteLength": len(out_blob)}]
    save_glb(output_path, out_js, out_blob)
    print(f"wrote {output_path}")


def main() -> None:
    if not HUMAN_MALE.is_file():
        raise SystemExit(f"Missing human male GLB: {HUMAN_MALE}")
    if not HUMAN_FEMALE_SOURCE.is_file():
        raise SystemExit(f"Missing human female source GLB: {HUMAN_FEMALE_SOURCE}")

    build_retargeted_human(HUMAN_MALE, HUMAN_MALE)
    build_retargeted_human(HUMAN_FEMALE_SOURCE, HUMAN_FEMALE)

    LICENSE_DST.parent.mkdir(parents=True, exist_ok=True)
    src_license = UAL_GLB.parent.parent / "License.txt"
    if src_license.is_file():
        shutil.copy2(src_license, LICENSE_DST)
        print(f"copied license to {LICENSE_DST}")


if __name__ == "__main__":
    main()
