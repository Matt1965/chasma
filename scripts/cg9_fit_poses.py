#!/usr/bin/env python3
"""Deterministic CG9 equipment clearance test poses."""

from __future__ import annotations

# Semantic appearance values (0..1). Default for all params is 0.5 (neutral).
SEMANTIC_DEFAULT = 0.5

BODY_SEMANTIC_PARAMS = (
    "build",
    "fat",
    "muscle",
    "head_size",
    "shoulders",
    "torso",
    "arms",
    "hips",
    "legs",
)

EQUIPMENT_CONSUMED: dict[str, tuple[str, ...]] = {
    "peasant_body": ("build", "fat", "muscle", "shoulders", "torso", "hips"),
    "peasant_arms": ("build", "fat", "muscle", "arms"),
    "peasant_legs": ("build", "fat", "muscle", "legs", "hips"),
    "ranger_body": ("build", "fat", "muscle", "shoulders", "torso", "hips"),
    "ranger_arms": ("build", "fat", "muscle", "arms"),
    "ranger_legs": ("build", "fat", "muscle", "legs", "hips"),
    "ranger_hood": ("head_size",),
}

# Workbook human morph mappings (default 0.5).
HUMAN_MORPH_MAP: dict[str, tuple[tuple[str, str], ...]] = {
    "build": (("build_broad", "above"), ("build_narrow", "below")),
    "fat": (("fat_soft", "above"),),
    "muscle": (("muscle_define", "above"),),
    "head_size": (("head_large", "above"), ("head_small", "below")),
    "shoulders": (("shoulders_broad", "above"), ("shoulders_narrow", "below")),
    "torso": (("torso_broad", "above"), ("torso_narrow", "below")),
    "arms": (("arms_thick", "above"), ("arms_thin", "below")),
    "hips": (("hips_broad", "above"), ("hips_narrow", "below")),
    "legs": (("legs_thick", "above"), ("legs_thin", "below")),
}


def semantic_deviation(value: float, side: str) -> float:
    default = SEMANTIC_DEFAULT
    if side == "above":
        if value <= default:
            return 0.0
        return (value - default) / (1.0 - default)
    if value >= default:
        return 0.0
    return (default - value) / default


def resolve_morph_weights(
    semantic_values: dict[str, float],
    target_names: list[str],
    consumed: tuple[str, ...] | None = None,
) -> dict[str, float]:
    weights = {name: 0.0 for name in target_names}
    allowed = set(consumed) if consumed is not None else set(HUMAN_MORPH_MAP)
    for param, mappings in HUMAN_MORPH_MAP.items():
        if param not in allowed:
            continue
        value = semantic_values.get(param, SEMANTIC_DEFAULT)
        for target, side in mappings:
            if target not in weights:
                continue
            weights[target] = min(1.0, weights[target] + semantic_deviation(value, side))
    return weights


def neutral_semantics() -> dict[str, float]:
    return {param: SEMANTIC_DEFAULT for param in BODY_SEMANTIC_PARAMS}


def pose_matrix() -> dict[str, dict[str, float]]:
    base = neutral_semantics()
    poses: dict[str, dict[str, float]] = {"neutral": dict(base)}

    def set_pose(name: str, **kwargs: float) -> None:
        pose = dict(base)
        pose.update(kwargs)
        poses[name] = pose

    set_pose("build_0", build=0.0)
    set_pose("build_1", build=1.0)
    set_pose("fat_1", fat=1.0)
    set_pose("muscle_1", muscle=1.0)
    set_pose("shoulders_0", shoulders=0.0)
    set_pose("shoulders_1", shoulders=1.0)
    set_pose("torso_0", torso=0.0)
    set_pose("torso_1", torso=1.0)
    set_pose("arms_0", arms=0.0)
    set_pose("arms_1", arms=1.0)
    set_pose("hips_0", hips=0.0)
    set_pose("hips_1", hips=1.0)
    set_pose("legs_0", legs=0.0)
    set_pose("legs_1", legs=1.0)
    set_pose("head_0", head_size=0.0)
    set_pose("head_1", head_size=1.0)

    set_pose("shoulders1_muscle1", shoulders=1.0, muscle=1.0)
    set_pose("narrow_shoulders_broad_build", shoulders=0.0, build=1.0)
    set_pose("torso1_fat1", torso=1.0, fat=1.0)
    set_pose("arms1_muscle1", arms=1.0, muscle=1.0)
    set_pose("hips1_fat1", hips=1.0, fat=1.0)
    set_pose("legs1_muscle1", legs=1.0, muscle=1.0)
    set_pose("narrow_build_broad_hips", build=0.0, hips=1.0)
    set_pose("shoulders1_torso1_arms1", shoulders=1.0, torso=1.0, arms=1.0)
    set_pose("hips1_legs1_fat1", hips=1.0, legs=1.0, fat=1.0)
    return poses
