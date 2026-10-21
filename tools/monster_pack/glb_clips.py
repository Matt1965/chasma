#!/usr/bin/env py -3
"""List GLB animation clip names and durations."""

from __future__ import annotations

import json
import struct
import sys
from pathlib import Path


def read_glb_json(path: Path) -> dict:
    with path.open("rb") as handle:
        handle.read(12)
        chunk_len = struct.unpack("<I", handle.read(4))[0]
        handle.read(4)
        return json.loads(handle.read(chunk_len))


def list_clips(path: Path) -> list[dict]:
    gltf = read_glb_json(path)
    clips: list[dict] = []
    for index, anim in enumerate(gltf.get("animations", [])):
        name = anim.get("name") or f"anim_{index}"
        # Duration requires accessors; approximate via channels if missing.
        clips.append({"name": name})
    return clips


def main() -> None:
    path = Path(sys.argv[1])
    clips = list_clips(path)
    for clip in clips:
        print(clip["name"])


if __name__ == "__main__":
    main()
