#!/usr/bin/env python3
"""Presentation bind checks for monster-pack GLBs (catalog correction × glTF hierarchy)."""

from __future__ import annotations

import math
from pathlib import Path

from glb_sanitize_animations import load_glb

# glTF quaternion [x, y, z, w]
IDENTITY_QUAT = [0.0, 0.0, 0.0, 1.0]


def quat_mul(a: list[float], b: list[float]) -> list[float]:
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]


def quat_rotate(q: list[float], v: list[float]) -> list[float]:
    x, y, z, w = q
    vx, vy, vz = v
    qx, qy, qz = x, y, z
    cx = qy * vz - qz * vy
    cy = qz * vx - qx * vz
    cz = qx * vy - qy * vx
    return [
        vx + 2 * (w * cx + qy * cz - qz * cy),
        vy + 2 * (w * cy + qz * cx - qx * cz),
        vz + 2 * (w * cz + qx * cy - qy * cx),
    ]


def quat_from_axis_angle_deg(axis: tuple[float, float, float], degrees: float) -> list[float]:
    ax, ay, az = axis
    half = math.radians(degrees) * 0.5
    s = math.sin(half)
    return [ax * s, ay * s, az * s, math.cos(half)]


def catalog_correction_quat(pitch_x_deg: float, yaw_y_deg: float, roll_z_deg: float) -> list[float]:
    """Match Chasma workbook mapping: yaw=Y, pitch=X, roll=Z (YXZ application order)."""
    qx = quat_from_axis_angle_deg((1.0, 0.0, 0.0), pitch_x_deg)
    qy = quat_from_axis_angle_deg((0.0, 1.0, 0.0), yaw_y_deg)
    qz = quat_from_axis_angle_deg((0.0, 0.0, 1.0), roll_z_deg)
    return quat_mul(quat_mul(qy, qx), qz)


def parent_map(gltf: dict) -> dict[int, int]:
    parents: dict[int, int] = {}
    for pi, pn in enumerate(gltf["nodes"]):
        for c in pn.get("children", []):
            parents[c] = pi
    return parents


def world_rotation(
    gltf: dict,
    idx: int,
    parents: dict[int, int],
    cache: dict[int, list[float]],
) -> list[float]:
    if idx in cache:
        return cache[idx]
    n = gltf["nodes"][idx]
    r = list(n.get("rotation", IDENTITY_QUAT))
    if idx not in parents:
        cache[idx] = r
        return r
    pr = world_rotation(gltf, parents[idx], parents, cache)
    cache[idx] = quat_mul(pr, r)
    return cache[idx]


def presentation_up(
    gltf: dict,
    body_bone_name: str,
    catalog_pitch_x_deg: float,
    catalog_yaw_y_deg: float,
    catalog_roll_z_deg: float,
) -> list[float]:
    parents = parent_map(gltf)
    body_index = next(
        i
        for i, n in enumerate(gltf["nodes"])
        if (n.get("name") or "").strip() == body_bone_name
    )
    cache: dict[int, list[float]] = {}
    bind_world = world_rotation(gltf, body_index, parents, cache)
    catalog = catalog_correction_quat(catalog_pitch_x_deg, catalog_yaw_y_deg, catalog_roll_z_deg)
    visual_world = quat_mul(catalog, bind_world)
    return quat_rotate(visual_world, [0.0, 1.0, 0.0])


def verify_presentation_upright(
    path: Path,
    body_bone_name: str,
    catalog_pitch_x_deg: float = 0.0,
    catalog_yaw_y_deg: float = 180.0,
    catalog_roll_z_deg: float = 0.0,
    min_up_y: float = 0.9,
) -> tuple[bool, list[float]]:
    gltf, _ = load_glb(path)
    up = presentation_up(gltf, body_bone_name, catalog_pitch_x_deg, catalog_yaw_y_deg, catalog_roll_z_deg)
    ok = up[1] >= min_up_y and abs(up[0]) < 0.2 and abs(up[2]) < 0.2
    return ok, up
