"""Linear-blend skinning evaluator for GLB exports (glTF 2.0 semantics, matches Bevy).

For each skinned primitive: `skinned(p) = sum_k w_k * (G_joint_k * IBM_k) * p` is compared with
the mesh node's own global transform applied to `p`. In rest pose a correct export gives ~0
deviation. The whole scene is evaluated under a world offset (default 300 m from origin) so
weight-sum defects - which vanish at the origin - are caught.

Also checks the raw invariants the renderer relies on: a single influence set, weights
summing to 1, joint indices inside the skin, and inverse bind matrices present.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from glb_sanitize_animations import load_glb  # noqa: E402
from gltf_skin_influences import read_accessor  # noqa: E402

DEFAULT_WORLD_OFFSET = (300.0, 50.0, 120.0)
WEIGHT_SUM_TOLERANCE = 1e-3


def quat_to_mat3(q) -> np.ndarray:
    x, y, z, w = q
    return np.array(
        [
            [1 - 2 * (y * y + z * z), 2 * (x * y - w * z), 2 * (x * z + w * y)],
            [2 * (x * y + w * z), 1 - 2 * (x * x + z * z), 2 * (y * z - w * x)],
            [2 * (x * z - w * y), 2 * (y * z + w * x), 1 - 2 * (x * x + y * y)],
        ]
    )


def trs_matrix(t, r, s) -> np.ndarray:
    m = np.eye(4)
    m[:3, :3] = quat_to_mat3(r) * np.array(s)[None, :]
    m[:3, 3] = t
    return m


def node_rest_trs(node: dict):
    if "matrix" in node:
        raise ValueError("node uses `matrix`; TRS expected (FBX2glTF emits TRS)")
    return (
        np.array(node.get("translation", [0, 0, 0]), dtype=np.float64),
        np.array(node.get("rotation", [0, 0, 0, 1]), dtype=np.float64),
        np.array(node.get("scale", [1, 1, 1]), dtype=np.float64),
    )


def parent_map(gltf: dict) -> dict[int, int]:
    parents: dict[int, int] = {}
    for i, node in enumerate(gltf["nodes"]):
        for child in node.get("children", []):
            parents[child] = i
    return parents


def global_matrices(gltf: dict, local_trs: dict[int, tuple], world: np.ndarray) -> list[np.ndarray]:
    parents = parent_map(gltf)
    cache: dict[int, np.ndarray] = {}

    def resolve(i: int) -> np.ndarray:
        if i in cache:
            return cache[i]
        local = trs_matrix(*local_trs[i])
        parent = parents.get(i)
        cache[i] = (resolve(parent) if parent is not None else world) @ local
        return cache[i]

    return [resolve(i) for i in range(len(gltf["nodes"]))]


def _sample(times: np.ndarray, values: np.ndarray, t: float, rotation: bool) -> np.ndarray:
    if t <= times[0]:
        return values[0]
    if t >= times[-1]:
        return values[-1]
    k = int(np.searchsorted(times, t, side="right") - 1)
    t0, t1 = times[k], times[k + 1]
    a = 0.0 if t1 <= t0 else (t - t0) / (t1 - t0)
    v0, v1 = values[k], values[k + 1]
    if rotation:
        if np.dot(v0, v1) < 0:
            v1 = -v1
        out = (1 - a) * v0 + a * v1
        return out / np.linalg.norm(out)
    return (1 - a) * v0 + a * v1


def pose_at(gltf: dict, blob: bytes, animation: dict | None, t: float) -> dict[int, tuple]:
    pose = {i: node_rest_trs(n) for i, n in enumerate(gltf["nodes"])}
    if animation is None:
        return pose
    for channel in animation["channels"]:
        target = channel["target"]
        node = target.get("node")
        if node is None or target["path"] not in ("translation", "rotation", "scale"):
            continue
        sampler = animation["samplers"][channel["sampler"]]
        times = read_accessor(gltf, blob, sampler["input"]).astype(np.float64).reshape(-1)
        values = read_accessor(gltf, blob, sampler["output"]).astype(np.float64)
        if sampler.get("interpolation") == "CUBICSPLINE":
            values = values.reshape(len(times), 3, -1)[:, 1, :]
        value = _sample(times, values, t, target["path"] == "rotation")
        t_, r_, s_ = pose[node]
        pose[node] = {
            "translation": (value, r_, s_),
            "rotation": (t_, value, s_),
            "scale": (t_, r_, value),
        }[target["path"]]
    return pose


def animation_duration(gltf: dict, blob: bytes, animation: dict) -> float:
    end = 0.0
    for sampler in animation["samplers"]:
        times = read_accessor(gltf, blob, sampler["input"]).reshape(-1)
        if len(times):
            end = max(end, float(times[-1]))
    return end


def skin_invariants(gltf: dict, blob: bytes) -> dict[str, Any]:
    """Raw renderer invariants, independent of pose."""
    issues: list[str] = []
    primitives: list[dict[str, Any]] = []
    mesh_skin = {n["mesh"]: n.get("skin") for n in gltf["nodes"] if n.get("mesh") is not None}
    for mesh_index, mesh in enumerate(gltf.get("meshes", [])):
        for prim_index, prim in enumerate(mesh.get("primitives", [])):
            attrs = prim["attributes"]
            if "JOINTS_0" not in attrs:
                continue
            label = f"{mesh.get('name')}[{prim_index}]"
            skin_index = mesh_skin.get(mesh_index)
            if skin_index is None:
                issues.append(f"{label}: skinned primitive on a node without `skin`")
                continue
            skin = gltf["skins"][skin_index]
            if skin.get("inverseBindMatrices") is None:
                issues.append(f"{label}: skin {skin_index} has no inverseBindMatrices")
            extra_sets = sorted(k for k in attrs if k.startswith(("JOINTS_", "WEIGHTS_")) and not k.endswith("_0"))
            if extra_sets:
                issues.append(f"{label}: extra influence sets {extra_sets} (renderer reads set 0 only)")
            weights = read_accessor(gltf, blob, attrs["WEIGHTS_0"]).astype(np.float64)
            joints = read_accessor(gltf, blob, attrs["JOINTS_0"]).astype(np.int64)
            sums = weights.sum(axis=1)
            off = int((np.abs(sums - 1.0) > WEIGHT_SUM_TOLERANCE).sum())
            if off:
                issues.append(
                    f"{label}: {off}/{len(sums)} vertices with weight sum outside 1 +/- {WEIGHT_SUM_TOLERANCE} "
                    f"(min {sums.min():.4f})"
                )
            used = joints[weights > 0]
            if used.size and int(used.max()) >= len(skin["joints"]):
                issues.append(f"{label}: joint index {int(used.max())} >= skin joint count {len(skin['joints'])}")
            primitives.append(
                {
                    "primitive": label,
                    "vertices": int(len(sums)),
                    "skin": skin_index,
                    "skin_joints": len(skin["joints"]),
                    "weight_sum_min": float(sums.min()),
                    "weight_sum_max": float(sums.max()),
                    "extra_sets": extra_sets,
                }
            )
    return {"ok": not issues, "issues": issues, "primitives": primitives}


def evaluate_pose(
    gltf: dict,
    blob: bytes,
    pose: dict[int, tuple],
    world_offset=DEFAULT_WORLD_OFFSET,
    *,
    keep_positions: bool = False,
) -> list[dict[str, Any]]:
    world = np.eye(4)
    world[:3, 3] = world_offset
    globals_ = global_matrices(gltf, pose, world)
    rows: list[dict[str, Any]] = []
    for node_index, node in enumerate(gltf["nodes"]):
        if node.get("mesh") is None or node.get("skin") is None:
            continue
        skin = gltf["skins"][node["skin"]]
        if skin.get("inverseBindMatrices") is None:
            continue
        ibm = read_accessor(gltf, blob, skin["inverseBindMatrices"]).astype(np.float64)
        # glTF MAT4 accessors are column-major: transpose each 16-float block.
        ibm = ibm.reshape(-1, 4, 4).transpose(0, 2, 1)
        joint_mats = np.stack([globals_[j] @ ibm[k] for k, j in enumerate(skin["joints"])])
        mesh_mat = globals_[node_index]
        for prim_index, prim in enumerate(gltf["meshes"][node["mesh"]]["primitives"]):
            attrs = prim["attributes"]
            if "JOINTS_0" not in attrs:
                continue
            pos = read_accessor(gltf, blob, attrs["POSITION"]).astype(np.float64)
            joints = read_accessor(gltf, blob, attrs["JOINTS_0"]).astype(np.int64)
            weights = read_accessor(gltf, blob, attrs["WEIGHTS_0"]).astype(np.float64)
            hom = np.concatenate([pos, np.ones((len(pos), 1))], axis=1)
            reference = (mesh_mat @ hom.T).T[:, :3]
            skinned = np.zeros_like(reference)
            for k in range(joints.shape[1]):
                m = joint_mats[np.clip(joints[:, k], 0, len(joint_mats) - 1)]
                moved = np.einsum("vij,vj->vi", m, hom)[:, :3]
                skinned += weights[:, k : k + 1] * moved
            dev = np.linalg.norm(skinned - reference, axis=1)
            extent = float((reference.max(axis=0) - reference.min(axis=0)).max())
            row = {
                "primitive": f"{gltf['meshes'][node['mesh']].get('name')}[{prim_index}]",
                "vertices": int(len(dev)),
                "max_dev_m": float(dev.max()),
                "median_dev_m": float(np.median(dev)),
                "extent_m": extent,
            }
            if keep_positions:
                row["skinned"] = skinned
            rows.append(row)
    return rows


def translation_invariance(gltf: dict, blob: bytes, world_offset=DEFAULT_WORLD_OFFSET) -> dict[str, Any]:
    """Skinned positions must move rigidly with the scene: skin(offset) - skin(origin) == offset.

    Any per-vertex violation means the renderer's output depends on where the unit stands,
    which is exactly the weight-sum / out-of-range-joint failure mode.
    """
    pose = pose_at(gltf, blob, None, 0.0)
    at_origin = evaluate_pose(gltf, blob, pose, (0.0, 0.0, 0.0), keep_positions=True)
    at_offset = evaluate_pose(gltf, blob, pose, world_offset, keep_positions=True)
    offset = np.array(world_offset, dtype=np.float64)
    worst = 0.0
    worst_primitive = None
    for a, b in zip(at_origin, at_offset):
        err = float(np.linalg.norm((b["skinned"] - a["skinned"]) - offset, axis=1).max())
        if err > worst:
            worst, worst_primitive = err, a["primitive"]
    return {"max_error_m": worst, "primitive": worst_primitive, "world_offset": list(world_offset)}


def assess_glb(path: Path, *, world_offset=DEFAULT_WORLD_OFFSET, clip_fraction: float = 0.5, max_clips: int | None = None) -> dict[str, Any]:
    gltf, blob = load_glb(path)
    invariants = skin_invariants(gltf, blob)
    rest = evaluate_pose(gltf, blob, pose_at(gltf, blob, None, 0.0), world_offset)
    clips: list[dict[str, Any]] = []
    for anim in gltf.get("animations", [])[: max_clips or None]:
        duration = animation_duration(gltf, blob, anim)
        rows = evaluate_pose(gltf, blob, pose_at(gltf, blob, anim, duration * clip_fraction), world_offset)
        clips.append(
            {
                "clip": anim.get("name"),
                "sample_t": duration * clip_fraction,
                "max_dev_m": max((r["max_dev_m"] for r in rows), default=0.0),
                "max_extent_m": max((r["extent_m"] for r in rows), default=0.0),
            }
        )
    return {
        "glb": str(path),
        "world_offset": list(world_offset),
        "invariants": invariants,
        "translation_invariance": translation_invariance(gltf, blob, world_offset),
        "rest": rest,
        "rest_max_dev_m": max((r["max_dev_m"] for r in rest), default=0.0),
        "clips": clips,
    }


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("glb", type=Path, nargs="+")
    ap.add_argument("--max-clips", type=int, default=None)
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()
    for path in args.glb:
        report = assess_glb(path, max_clips=args.max_clips)
        if args.json:
            print(json.dumps(report, indent=2))
            continue
        print(f"== {path}")
        for issue in report["invariants"]["issues"]:
            print(f"  INVARIANT: {issue}")
        ti = report["translation_invariance"]
        print(f"  translation invariance max_error={ti['max_error_m']:.6f}m ({ti['primitive']})")
        for row in report["rest"]:
            print(f"  rest {row['primitive']:<44} verts={row['vertices']:<6} max_dev={row['max_dev_m']:.4f}m extent={row['extent_m']:.2f}m")
        for clip in report["clips"]:
            print(f"  clip {clip['clip']:<32} t={clip['sample_t']:.2f}s max_dev={clip['max_dev_m']:.3f}m")


if __name__ == "__main__":
    main()
