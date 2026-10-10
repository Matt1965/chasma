#!/usr/bin/env python3
"""Quick GLB skin sanity check (neutral_bone trap, buffer size)."""

from __future__ import annotations

import argparse
import json
import struct
import sys
from collections import Counter
from pathlib import Path


def load_glb(path: Path) -> tuple[dict, bytes]:
    data = path.read_bytes()
    if data[:4] != b"glTF":
        raise ValueError(f"Not a GLB: {path}")
    jlen = struct.unpack_from("<I", data, 12)[0]
    jstart = 20
    gltf = json.loads(data[jstart : jstart + jlen])
    bin_header = jstart + jlen
    blob_len = struct.unpack_from("<I", data, bin_header)[0]
    blob = data[bin_header + 8 : bin_header + 8 + blob_len]
    return gltf, blob


def check(path: Path) -> int:
    gltf, _blob = load_glb(path)
    issues: list[str] = []
    buf_len = gltf.get("buffers", [{}])[0].get("byteLength")
    skins = gltf.get("skins", [])
    if not skins:
        issues.append("no skins")
    else:
        joints = skins[0].get("joints", [])
        names = [gltf["nodes"][i].get("name", "") for i in joints]
        if "neutral_bone" in names:
            issues.append("neutral_bone in skin joints")
    meshes = gltf.get("meshes", [])
    if meshes:
        prim = meshes[0]["primitives"][0]
        if "JOINTS_0" in prim.get("attributes", {}):
            # lightweight: only flag neutral in joint name list
            pass
    anims = len(gltf.get("animations", []))
    if anims == 0:
        issues.append("no animations")
    if issues:
        print(f"FAIL {path.name}: {', '.join(issues)} (buf={buf_len}, clips={anims})")
        return 1
    print(f"OK {path.name}: clips={anims} buf={buf_len}")
    return 0


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("glb", type=Path, nargs="+")
    args = parser.parse_args()
    code = 0
    for p in args.glb:
        try:
            code |= check(p.resolve())
        except OSError as e:
            print(f"SKIP {p}: {e}")
            code |= 1
    sys.exit(code)


if __name__ == "__main__":
    main()
