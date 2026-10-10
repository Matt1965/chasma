#!/usr/bin/env python3
"""Heuristic: detect GLBs likely exported with child channel stripping."""

from __future__ import annotations

import argparse
import json
import re
import struct
from pathlib import Path

from glb_sanitize_animations import load_glb

_TONGUE_RE = re.compile(r"Tongue\d*$", re.I)


def channel_stats(path: Path) -> dict:
    gltf, _ = load_glb(path)
    nodes = gltf.get("nodes", [])
    tongue = {i for i, n in enumerate(nodes) if _TONGUE_RE.search(n.get("name") or "")}
    collapsed_tongue = any(
        all(s < 0.01 for s in nodes[i].get("scale", [1, 1, 1])) for i in tongue
    )
    idle = next((a for a in gltf.get("animations", []) if a.get("name") == "IdleBreathe"), None)
    if idle is None:
        idle = next(
            (a for a in gltf.get("animations", []) if "idle" in (a.get("name") or "").lower()),
            None,
        )
    child_trans = 0
    body_rot_idle = False
    tongue_ch_idle = 0
    if idle:
        for ch in idle.get("channels", []):
            target = ch.get("target", {})
            path_attr = target.get("path")
            node_index = target.get("node")
            if node_index is None:
                continue
            name = nodes[node_index].get("name") or ""
            if path_attr == "translation" and name not in ("root", "ROOT_", "Bufomorph_"):
                child_trans += 1
            if name == "Bufomorph_" and path_attr == "rotation":
                body_rot_idle = True
            if node_index in tongue and path_attr in ("rotation", "translation", "scale"):
                tongue_ch_idle += 1
    likely_stripped = child_trans == 0 and len(gltf.get("animations", [])) > 0
    return {
        "path": str(path),
        "animations": len(gltf.get("animations", [])),
        "collapsed_tongue_bind": collapsed_tongue,
        "idle_child_translation_channels": child_trans,
        "idle_body_rotation_channel": body_rot_idle,
        "idle_tongue_channels": tongue_ch_idle,
        "likely_child_translation_strip_export": likely_stripped,
        "likely_bufomorph_workaround_glb": collapsed_tongue,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("glb", type=Path, nargs="+")
    parser.add_argument("--json", type=Path, default=None)
    args = parser.parse_args()
    rows = [channel_stats(p) for p in args.glb]
    for row in rows:
        print(row)
    if args.json:
        args.json.write_text(json.dumps(rows, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
