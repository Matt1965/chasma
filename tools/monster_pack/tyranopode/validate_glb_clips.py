#!/usr/bin/env py -3
"""Validate Tyranopode GLB clips: duration, channels, and bone motion deltas."""

from __future__ import annotations

import argparse
import json
import struct
import sys
from pathlib import Path

import numpy as np


def load_glb(path: Path) -> tuple[dict, bytes]:
    data = path.read_bytes()
    if data[:4] != b"glTF":
        raise SystemExit(f"Not a GLB: {path}")
    jlen = struct.unpack_from("<I", data, 12)[0]
    jstart = 20
    gltf = json.loads(data[jstart : jstart + jlen])
    blob_start = jstart + jlen
    blob_len = struct.unpack_from("<I", data, blob_start + 4)[0]
    blob = data[blob_start + 12 : blob_start + 12 + blob_len]
    return gltf, blob


def read_accessor(gltf: dict, blob: bytes, index: int) -> np.ndarray:
    acc = gltf["accessors"][index]
    bv = gltf["bufferViews"][acc["bufferView"]]
    start = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    end = start + bv["byteLength"]
    raw = blob[start:end]
    dtype = {5126: np.float32, 5123: np.uint16, 5125: np.uint32}[acc["componentType"]]
    count = acc["count"]
    comps = {"SCALAR": 1, "VEC3": 3, "VEC4": 4, "MAT4": 16}[acc["type"]]
    arr = np.frombuffer(raw, dtype=dtype, count=count * comps)
    return arr.reshape((count, comps)) if comps > 1 else arr


def clip_motion_report(gltf: dict, blob: bytes, clip_name: str) -> dict:
    nodes = gltf["nodes"]
    anims = gltf.get("animations", [])
    clip = next((a for a in anims if a.get("name") == clip_name), None)
    if clip is None:
        return {"clip": clip_name, "error": "missing"}

    max_trans = 0.0
    max_rot = 0.0
    channels = 0
    for ch in clip["channels"]:
        channels += 1
        path = ch["target"]["path"]
        sampler = clip["samplers"][ch["sampler"]]
        values = read_accessor(gltf, blob, sampler["output"])
        if path == "translation":
            max_trans = max(max_trans, float(np.max(np.linalg.norm(values, axis=1))))
        elif path == "rotation":
            max_rot = max(max_rot, float(np.max(np.linalg.norm(values[:, 1:], axis=1))))

    times = read_accessor(gltf, blob, clip["samplers"][0]["input"])
    duration = float(times[-1] - times[0]) if len(times) else 0.0
    return {
        "clip": clip_name,
        "channels": channels,
        "duration_s": round(duration, 3),
        "max_translation": round(max_trans, 4),
        "max_rotation_delta": round(max_rot, 4),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("glb", type=Path, default=Path("assets/units/tyranopode.glb"))
    parser.add_argument("--profile-clips", nargs="*", default=["Idle", "CrawlForward", "CrawlLeft", "Death", "GetHit"])
    args = parser.parse_args()
    gltf, blob = load_glb(args.glb.resolve())
    names = [a.get("name", "") for a in gltf.get("animations", [])]
    print(f"{args.glb}: {len(names)} clips")
    for name in args.profile_clips:
        print(clip_motion_report(gltf, blob, name))
    print("all clips:", ", ".join(names))


if __name__ == "__main__":
    main()
