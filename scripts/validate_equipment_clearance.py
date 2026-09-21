#!/usr/bin/env python3
"""Validate body/equipment clearance for the CG9 fit matrix."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

from author_equipment_local_fit import (
    EQUIPMENT_PIECES,
    PENETRATION_TOLERANCE_M,
    body_primitive_cache,
    clearance_corrections,
    morphed_body_surface_from_cache,
)
from cg9_fit_poses import (
    DEFAULT_MIN_CLEARANCE_M,
    EQUIPMENT_CONSUMED,
    HOOD_MIN_CLEARANCE_M,
    min_clearance_m,
    pose_matrix,
    resolve_morph_weights,
)
from glb_geometry import (
    BodySampleGrid,
    all_skinned_primitives,
    apply_morph_weights,
    load_glb,
    primary_skinned_primitive,
)

REPO = Path(__file__).resolve().parents[1]
ASSETS = REPO / "assets"


def measure_clearance_cached(
    body_cache: dict,
    equip,
    consumed: tuple[str, ...],
    semantics: dict[str, float],
    clearance_target: float,
) -> dict:
    body_weights = resolve_morph_weights(semantics, body_cache["target_names"])
    equip_weights = resolve_morph_weights(semantics, equip.target_names, consumed)
    body_key = tuple(sorted((k, round(v, 4)) for k, v in body_weights.items() if v > 0))
    if body_key not in body_cache["surfaces"]:
        body_cache["surfaces"][body_key] = morphed_body_surface_from_cache(
            body_cache["prims"], body_weights
        )
    body_verts, body_normals = body_cache["surfaces"][body_key]
    if body_key not in body_cache["grids"]:
        stride = max(1, len(body_verts) // 2500)
        body_cache["grids"][body_key] = BodySampleGrid(body_verts[::stride])
    equip_verts = apply_morph_weights(equip.positions, equip.target_deltas, equip_weights)
    needed = clearance_corrections(
        body_verts,
        body_normals,
        equip_verts,
        equip.normals,
        body_cache["grids"][body_key],
        clearance_target,
    )
    needed_mag = np.linalg.norm(needed, axis=1)
    penetrating = int(np.sum(needed_mag > PENETRATION_TOLERANCE_M))
    min_clearance = (
        float(clearance_target - np.max(needed_mag)) if len(needed_mag) else clearance_target
    )
    return {
        "clearance_target_m": clearance_target,
        "penetrating_vertices": penetrating,
        "min_clearance_m": min_clearance,
        "max_needed_correction_m": float(np.max(needed_mag)) if len(needed_mag) else 0.0,
    }


def main() -> int:
    poses = pose_matrix()
    summary = {
        "min_clearance_target_m": DEFAULT_MIN_CLEARANCE_M,
        "hood_min_clearance_target_m": HOOD_MIN_CLEARANCE_M,
        "penetration_tolerance_m": PENETRATION_TOLERANCE_M,
        "cases": {},
        "failures": [],
    }
    worst_min = 1e9
    worst_case = ""

    for unit_key in ("human_male", "human_female"):
        body_path = ASSETS / "units" / f"{unit_key}.glb"
        body_js, body_blob = load_glb(body_path)
        body_prims = all_skinned_primitives(body_js, body_blob)
        body_cache = {
            "prims": body_prims,
            "target_names": body_prims[0].target_names,
            "surfaces": {},
            "grids": {},
        }
        for asset_name in EQUIPMENT_PIECES:
            equip_path = ASSETS / "items" / "equipment" / unit_key / f"{asset_name}.glb"
            equip_js, equip_blob = load_glb(equip_path)
            equip = primary_skinned_primitive(equip_js, equip_blob)
            if equip is None:
                raise ValueError(f"missing equipment primitive: {equip_path}")
            consumed = EQUIPMENT_CONSUMED[asset_name]
            clearance_target = min_clearance_m(asset_name)
            piece_poses = poses
            if asset_name == "ranger_hood":
                piece_poses = {k: v for k, v in poses.items() if "head" in k or k == "neutral"}
            for pose_name, semantics in piece_poses.items():
                key = f"{unit_key}/{asset_name}/{pose_name}"
                metrics = measure_clearance_cached(
                    body_cache, equip, consumed, semantics, clearance_target
                )
                summary["cases"][key] = metrics
                if metrics["min_clearance_m"] < worst_min:
                    worst_min = metrics["min_clearance_m"]
                    worst_case = key
                if metrics["penetrating_vertices"] > 0:
                    summary["failures"].append(key)

    summary["total_penetrating_vertices"] = sum(
        m["penetrating_vertices"] for m in summary["cases"].values()
    )
    summary["worst_min_clearance_m"] = worst_min
    summary["worst_case"] = worst_case
    out = ASSETS / "items" / "equipment" / "clearance_validation.json"
    out.write_text(json.dumps(summary, indent=2))
    print(f"wrote {out}")
    if summary["failures"]:
        print(f"FAIL {len(summary['failures'])} cases")
        for key in summary["failures"][:10]:
            print(" ", key, summary["cases"][key])
        return 1
    print("PASS all cases")
    return 0


if __name__ == "__main__":
    sys.exit(main())
