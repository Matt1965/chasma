"""Point every skinned mesh at one canonical skin; remap JOINTS_0 indices by joint node id."""

from __future__ import annotations

import struct
from pathlib import Path
from typing import Any

from glb_sanitize_animations import load_glb, write_glb

_JOINT_COMPONENT = {5121: ("<BBBB", 1, 5121), 5123: ("<HHHH", 2, 5123)}


def _read_joints(blob, gltf, acc_index):
    acc = gltf["accessors"][acc_index]
    fmt, mul, _ = _JOINT_COMPONENT[acc["componentType"]]
    bv = gltf["bufferViews"][acc["bufferView"]]
    off = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = bv.get("byteStride", struct.calcsize(fmt)) or struct.calcsize(fmt)
    out = []
    for i in range(acc["count"]):
        raw = struct.unpack_from(fmt, blob, off + i * stride)
        out.append(tuple(int(v) for v in raw))
    return out, acc


def _append_joints_u8(blob: bytearray, joints: list[tuple[int, int, int, int]], gltf: dict) -> int:
    while len(blob) % 4:
        blob.append(0)
    off = len(blob)
    for j4 in joints:
        blob.extend(struct.pack("<BBBB", *[min(255, j) for j in j4]))
    gltf["bufferViews"].append({"buffer": 0, "byteOffset": off, "byteLength": len(joints) * 4})
    gltf["accessors"].append(
        {"bufferView": len(gltf["bufferViews"]) - 1, "componentType": 5121, "count": len(joints), "type": "VEC4"}
    )
    return len(gltf["accessors"]) - 1


def unify_glb_skin(path: Path, out_path: Path | None = None) -> dict[str, Any]:
    gltf, blob = load_glb(path)
    blob = bytearray(blob)
    skins = gltf.get("skins", [])
    if len(skins) < 2:
        return {"unified": False, "reason": "single skin"}
    canonical = max(range(len(skins)), key=lambda i: len(skins[i]["joints"]))
    joint_to_canonical = {node: idx for idx, node in enumerate(skins[canonical]["joints"])}
    remapped_meshes = 0
    for node in gltf["nodes"]:
        if node.get("mesh") is None or node.get("skin") is None:
            continue
        local_skin = node["skin"]
        if local_skin == canonical:
            continue
        local_joints = skins[local_skin]["joints"]
        local_to_canonical = []
        for lj in local_joints:
            if lj not in joint_to_canonical:
                raise ValueError(f"joint node {lj} missing from canonical skin {canonical}")
            local_to_canonical.append(joint_to_canonical[lj])
        mesh = gltf["meshes"][node["mesh"]]
        for prim in mesh.get("primitives", []):
            attrs = prim.get("attributes", {})
            if "JOINTS_0" not in attrs:
                continue
            joints, _ = _read_joints(bytes(blob), gltf, attrs["JOINTS_0"])
            new_joints = []
            for j4 in joints:
                new_joints.append(tuple(local_to_canonical[j] if j < len(local_to_canonical) else 0 for j in j4))
            attrs["JOINTS_0"] = _append_joints_u8(blob, new_joints, gltf)
        node["skin"] = canonical
        remapped_meshes += 1
    gltf["buffers"][0]["byteLength"] = len(blob)
    target = out_path or path
    write_glb(target, gltf, bytes(blob))
    return {
        "unified": True,
        "canonical_skin": canonical,
        "joint_count": len(skins[canonical]["joints"]),
        "remapped_meshes": remapped_meshes,
    }
