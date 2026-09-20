#!/usr/bin/env python3
"""Apply local equipment clearance corrections against morphed body geometry.

Idempotent: re-running on already-corrected assets produces near-zero deltas when
clearance requirements are met.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

from cg9_fit_poses import (
    EQUIPMENT_CONSUMED,
    pose_matrix,
    resolve_morph_weights,
)
from glb_geometry import (
    BodySampleGrid,
    SkinnedPrimitive,
    all_skinned_primitives,
    apply_morph_weights,
    load_glb,
    primary_skinned_primitive,
    save_glb,
    triangle_indices,
    vertex_neighbors,
    write_skinned_primitive,
)

REPO = Path(__file__).resolve().parents[1]
ASSETS = REPO / "assets"

MIN_CLEARANCE_M = 0.003
MAX_CORRECTION_M = 0.020
PENETRATION_TOLERANCE_M = 0.001
SMOOTH_ITERS = 2
SMOOTH_ALPHA = 0.25
BODY_SAMPLE_RADIUS_M = 0.02
BODY_SUBSAMPLE = 2500
FIT_ITERS = 25

EQUIPMENT_PIECES = (
    "peasant_body",
    "peasant_arms",
    "peasant_legs",
    "ranger_body",
    "ranger_arms",
    "ranger_legs",
    "ranger_hood",
)


def body_primitive_cache(js: dict, blob: bytearray) -> list[SkinnedPrimitive]:
    return all_skinned_primitives(js, blob)


def morphed_body_surface_from_cache(
    prims: list[SkinnedPrimitive], weights: dict[str, float]
) -> tuple[np.ndarray, np.ndarray]:
    positions: list[np.ndarray] = []
    normals: list[np.ndarray] = []
    for prim in prims:
        positions.append(apply_morph_weights(prim.positions, prim.target_deltas, weights))
        normals.append(prim.normals)
    return np.concatenate(positions, axis=0), np.concatenate(normals, axis=0)


def body_samples(body_verts: np.ndarray, body_grid: BodySampleGrid | None = None) -> np.ndarray:
    if body_grid is not None:
        return body_grid.verts
    stride = max(1, len(body_verts) // BODY_SUBSAMPLE)
    return body_verts[::stride]


def oriented_equipment_normals(
    equip_verts: np.ndarray,
    equip_normals: np.ndarray,
    body_sample: np.ndarray,
) -> np.ndarray:
    equip_n = equip_normals / (np.linalg.norm(equip_normals, axis=1, keepdims=True) + 1e-8)
    rel_nn = equip_verts[:, None, :] - body_sample[None, :, :]
    nearest = body_sample[np.argmin(np.sum(rel_nn * rel_nn, axis=2), axis=1)]
    outward = equip_verts - nearest
    flip = np.sum(equip_n * outward, axis=1) < 0.0
    equip_n[flip] *= -1.0
    return equip_n


def clearance_corrections(
    body_verts: np.ndarray,
    body_normals: np.ndarray,
    equip_verts: np.ndarray,
    equip_normals: np.ndarray,
    body_grid: BodySampleGrid | None = None,
) -> np.ndarray:
    """Measure clearance along garment outward normal against nearby body samples."""
    corrections = np.zeros_like(equip_verts)
    sample = body_samples(body_verts, body_grid)
    equip_n = oriented_equipment_normals(equip_verts, equip_normals, sample)
    batch_size = 256
    for start in range(0, len(equip_verts), batch_size):
        end = min(len(equip_verts), start + batch_size)
        points = equip_verts[start:end]
        normals = equip_n[start:end]
        rel = points[:, None, :] - sample[None, :, :]
        dist = np.linalg.norm(rel, axis=2)
        near = dist < BODY_SAMPLE_RADIUS_M
        masked_dist = np.where(near, dist, np.inf)
        clearance = np.min(masked_dist, axis=1)
        needed = np.clip(MIN_CLEARANCE_M - clearance, 0.0, MAX_CORRECTION_M)
        for local_index, point in enumerate(points):
            if needed[local_index] <= 0.0 or not np.isfinite(clearance[local_index]):
                continue
            nearest_index = int(np.argmin(masked_dist[local_index]))
            direction = point - sample[nearest_index]
            direction_norm = float(np.linalg.norm(direction))
            if direction_norm < 1e-8:
                direction = normals[local_index]
            else:
                direction = direction / direction_norm
            corrections[start + local_index] = direction * needed[local_index]
    return corrections


def smooth_corrections(
    corrections: np.ndarray,
    neighbors: list[list[int]],
    active: np.ndarray,
) -> np.ndarray:
    out = corrections.copy()
    for _ in range(SMOOTH_ITERS):
        nxt = out.copy()
        for vertex_index, nbrs in enumerate(neighbors):
            if not active[vertex_index] or not nbrs:
                continue
            nbr_active = [j for j in nbrs if active[j]]
            if not nbr_active:
                continue
            avg = np.mean(out[nbr_active], axis=0)
            nxt[vertex_index] = (1.0 - SMOOTH_ALPHA) * out[vertex_index] + SMOOTH_ALPHA * avg
        out = nxt
    return out


def pose_correction(
    body_surface: tuple[np.ndarray, np.ndarray],
    equip: SkinnedPrimitive,
    consumed: tuple[str, ...],
    semantics: dict[str, float],
    body_grid: BodySampleGrid | None = None,
) -> np.ndarray:
    equip_weights = resolve_morph_weights(semantics, equip.target_names, consumed)
    body_verts, body_normals = body_surface
    equip_verts = apply_morph_weights(equip.positions, equip.target_deltas, equip_weights)
    return clearance_corrections(
        body_verts, body_normals, equip_verts, equip.normals, body_grid
    )


def body_surface_cache(
    body_prims: list[SkinnedPrimitive], poses: dict[str, dict[str, float]]
) -> dict[tuple, tuple[np.ndarray, np.ndarray]]:
    cache: dict[tuple, tuple[np.ndarray, np.ndarray]] = {}
    for semantics in poses.values():
        weights = resolve_morph_weights(semantics, body_prims[0].target_names)
        key = tuple(sorted((k, round(v, 4)) for k, v in weights.items() if v > 0))
        if key not in cache:
            cache[key] = morphed_body_surface_from_cache(body_prims, weights)
    return cache


def body_grid_cache(
    surfaces: dict[tuple, tuple[np.ndarray, np.ndarray]]
) -> dict[tuple, BodySampleGrid]:
    grids: dict[tuple, BodySampleGrid] = {}
    for key, (body_verts, _) in surfaces.items():
        stride = max(1, len(body_verts) // BODY_SUBSAMPLE)
        grids[key] = BodySampleGrid(body_verts[::stride])
    return grids


def pose_correction_batch(
    equip: SkinnedPrimitive,
    consumed: tuple[str, ...],
    poses: dict[str, dict[str, float]],
    surfaces: dict[tuple, tuple[np.ndarray, np.ndarray]],
    grids: dict[tuple, BodySampleGrid],
    body_prims: list[SkinnedPrimitive],
) -> list[tuple[dict[str, float], np.ndarray]]:
    pose_corrections: list[tuple[dict[str, float], np.ndarray]] = []
    for semantics in poses.values():
        weights = resolve_morph_weights(semantics, body_prims[0].target_names)
        key = tuple(sorted((k, round(v, 4)) for k, v in weights.items() if v > 0))
        pose_corrections.append(
            (semantics, pose_correction(surfaces[key], equip, consumed, semantics, grids[key]))
        )
    return pose_corrections


def worst_pose_correction(
    pose_corrections: list[tuple[dict[str, float], np.ndarray]],
) -> tuple[np.ndarray, float]:
    max_corr = np.zeros_like(pose_corrections[0][1])
    worst_needed = 0.0
    for _, corr in pose_corrections:
        magnitudes = np.linalg.norm(corr, axis=1)
        worst_needed = max(worst_needed, float(np.max(magnitudes)) if len(magnitudes) else 0.0)
        for vertex_index in range(len(corr)):
            if magnitudes[vertex_index] > np.linalg.norm(max_corr[vertex_index]):
                max_corr[vertex_index] = corr[vertex_index]
    return max_corr, worst_needed


def apply_pose_attributed_morph_correction(
    equip: SkinnedPrimitive,
    consumed: tuple[str, ...],
    pose_corrections: list[tuple[dict[str, float], np.ndarray]],
    neighbors: list[list[int]],
) -> float:
    combined = np.zeros_like(equip.positions)
    attribution: list[dict[str, float] | None] = [None] * len(equip.positions)
    for semantics, corr in pose_corrections:
        magnitudes = np.linalg.norm(corr, axis=1)
        for vertex_index, magnitude in enumerate(magnitudes):
            if magnitude > np.linalg.norm(combined[vertex_index]):
                combined[vertex_index] = corr[vertex_index]
                attribution[vertex_index] = semantics

    active = np.linalg.norm(combined, axis=1) > PENETRATION_TOLERANCE_M
    if not np.any(active):
        return 0.0
    smoothed = smooth_corrections(combined, neighbors, active)
    for vertex_index in range(len(smoothed)):
        if not active[vertex_index]:
            continue
        semantics = attribution[vertex_index]
        if semantics is None:
            equip.positions[vertex_index] += smoothed[vertex_index]
            continue
        equip_weights = resolve_morph_weights(semantics, equip.target_names, consumed)
        active_weight = sum(weight for weight in equip_weights.values() if weight > 0.01)
        if active_weight < 1e-6:
            equip.positions[vertex_index] += smoothed[vertex_index]
            continue
        share = smoothed[vertex_index] / active_weight
        for target_name, weight in equip_weights.items():
            if weight > 0.01:
                equip.target_deltas[target_name][vertex_index] += share
    return float(np.max(np.linalg.norm(smoothed, axis=1)))


def apply_local_fit(
    unit_key: str,
    asset_name: str,
    report: dict,
) -> None:
    body_path = ASSETS / "units" / f"{unit_key}.glb"
    equip_path = ASSETS / "items" / "equipment" / unit_key / f"{asset_name}.glb"
    consumed = EQUIPMENT_CONSUMED[asset_name]

    body_js, body_blob = load_glb(body_path)
    equip_js, equip_blob = load_glb(equip_path)
    equip = primary_skinned_primitive(equip_js, equip_blob)
    if equip is None:
        raise ValueError(f"no skinned primitive in {equip_path}")

    poses = pose_matrix()
    if asset_name == "ranger_hood":
        poses = {
            key: value
            for key, value in poses.items()
            if key in {"neutral", "head_0", "head_1", "build_1", "fat_1", "muscle_1"}
        }

    tris = triangle_indices(equip_js, equip_blob, equip.primitive)
    neighbors = vertex_neighbors(tris, len(equip.positions))
    body_prims = body_primitive_cache(body_js, body_blob)
    surfaces = body_surface_cache(body_prims, poses)
    grids = body_grid_cache(surfaces)

    penetrations_before = 0
    max_applied = 0.0
    final_worst = 1.0

    for _ in range(FIT_ITERS):
        surfaces = body_surface_cache(body_prims, poses)
        grids = body_grid_cache(surfaces)
        pose_corrections = pose_correction_batch(
            equip, consumed, poses, surfaces, grids, body_prims
        )
        _, final_worst = worst_pose_correction(pose_corrections)
        if final_worst < PENETRATION_TOLERANCE_M:
            break
        step = apply_pose_attributed_morph_correction(
            equip, consumed, pose_corrections, neighbors
        )
        penetrations_before += int(step > PENETRATION_TOLERANCE_M)
        max_applied = max(max_applied, step)
        if step > MAX_CORRECTION_M:
            raise ValueError(
                f"{unit_key}/{asset_name} requires correction > {MAX_CORRECTION_M}m; inspect morph transfer"
            )
    else:
        raise ValueError(
            f"{unit_key}/{asset_name} still needs {final_worst * 1000:.2f}mm after local fit"
        )

    mesh = equip_js["meshes"][equip.mesh_idx]
    extras = mesh.get("extras", {})
    if isinstance(extras, str):
        extras = json.loads(extras) if extras else {}
    extras["cg9LocalClearanceM"] = MIN_CLEARANCE_M
    extras["cg9MaxCorrectionM"] = MAX_CORRECTION_M
    mesh["extras"] = extras
    write_skinned_primitive(equip_js, equip_blob, equip)
    save_glb(equip_path, equip_js, equip_blob)

    report[f"{unit_key}/{asset_name}"] = {
        "max_correction_m": max_applied,
        "penetration_samples_before": penetrations_before,
        "poses_tested": len(poses),
        "final_worst_needed_m": final_worst,
    }
    print(
        f"local fit {unit_key}/{asset_name}: "
        f"max_corr={max_applied * 1000:.2f}mm "
        f"poses={len(poses)} final_worst={final_worst * 1000:.2f}mm"
    )


def main() -> None:
    report: dict = {
        "min_clearance_m": MIN_CLEARANCE_M,
        "max_correction_m": MAX_CORRECTION_M,
        "method": "nearest-surface distance clearance with body-to-equipment displacement",
        "smoothing": f"{SMOOTH_ITERS} iterations alpha={SMOOTH_ALPHA}",
    }
    targets = sys.argv[1:]
    if targets:
        for target in targets:
            path = Path(target)
            unit_key = path.parent.name
            asset_name = path.stem
            apply_local_fit(unit_key, asset_name, report)
    else:
        for unit_key in ("human_male", "human_female"):
            for asset_name in EQUIPMENT_PIECES:
                apply_local_fit(unit_key, asset_name, report)
    out = ASSETS / "items" / "equipment" / "clearance_validation.json"
    out.write_text(json.dumps(report, indent=2))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
