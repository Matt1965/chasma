//! Doodad instance collision shape resolution (ADR-098 DT2, ADR-126/129 AT3).
//!
//! Ground gameplay extents use one composition:
//! `authored collision meters × (definition baseline × instance)_xz`.

use bevy::prelude::*;

use crate::world::asset_sizing::DoodadCollisionShape;
use crate::world::authoring_transform::AuthoringScale;
use crate::world::occupancy::FootprintShape;
use crate::world::{DoodadCatalog, DoodadDefinition, DoodadRecord};

/// Resolved horizontal collision footprint for a doodad instance.
#[derive(Debug, Clone, PartialEq)]
pub struct DoodadInstanceCollision {
    pub blocks_movement: bool,
    pub shape: FootprintShape,
    pub yaw_radians: f32,
}

/// Composed XZ scale for doodad ground gameplay (baseline × instance X/Z).
pub fn doodad_composed_xz_scale(definition: &DoodadDefinition, instance_scale_xz: Vec2) -> Vec2 {
    let baseline = definition.asset_sizing.resolved_baseline_scale().to_vec3();
    Vec2::new(baseline.x * instance_scale_xz.x, baseline.z * instance_scale_xz.y)
}

/// Authored placement/pick radius (meters) before composed XZ scale.
pub fn doodad_authored_interaction_radius_meters(definition: &DoodadDefinition) -> f32 {
    definition
        .placement_radius_meters
        .max(definition.block_radius_meters)
        .max(0.0)
}

/// Interaction / pick radius in meters for a placed doodad (same compose as collision).
pub fn doodad_interaction_radius_meters(
    record: &DoodadRecord,
    definition: &DoodadDefinition,
) -> f32 {
    let base = doodad_authored_interaction_radius_meters(definition);
    let composed = doodad_composed_xz_scale(definition, record.placement.collision_scale_xz());
    base * composed.x.max(composed.y).max(0.0)
}

/// Definition-only placement radius (instance scale = 1) for Dev spawn spacing.
pub fn doodad_definition_placement_radius_meters(definition: &DoodadDefinition) -> f32 {
    let base = doodad_authored_interaction_radius_meters(definition);
    let composed = doodad_composed_xz_scale(definition, Vec2::ONE);
    base * composed.x.max(composed.y).max(0.0)
}

/// Ground collision uses yaw only; X/Z composed scale affects horizontal extents.
pub fn resolve_doodad_collision(
    record: &DoodadRecord,
    definition: &DoodadDefinition,
) -> DoodadInstanceCollision {
    let blocks = definition.blocks_movement;
    let yaw = record.placement.collision_yaw_radians();
    let combined_xz =
        doodad_composed_xz_scale(definition, record.placement.collision_scale_xz());

    if !blocks {
        return DoodadInstanceCollision {
            blocks_movement: false,
            shape: FootprintShape::Circle { radius_meters: 0.0 },
            yaw_radians: yaw,
        };
    }

    let shape = resolve_collision_shape(definition, combined_xz);
    DoodadInstanceCollision {
        blocks_movement: true,
        shape,
        yaw_radians: yaw,
    }
}

fn resolve_collision_shape(definition: &DoodadDefinition, scale_xz: Vec2) -> FootprintShape {
    let base_x = effective_base_radius_x(definition);
    let base_z = effective_base_radius_z(definition);
    let radius_x = base_x * scale_xz.x;
    let radius_z = base_z * scale_xz.y;
    let uniform_fallback = definition.block_radius_meters.max(0.0) * uniform_xz(scale_xz);

    match definition.collision_shape {
        // AT3: None is authored circle meters — must honor the same compose as Circle.
        DoodadCollisionShape::None | DoodadCollisionShape::Circle => {
            let uniform = if radius_x > 0.0 && radius_z > 0.0 {
                (radius_x + radius_z) * 0.5
            } else {
                uniform_fallback
            };
            FootprintShape::Circle {
                radius_meters: uniform.max(0.0),
            }
        }
        DoodadCollisionShape::Ellipse => FootprintShape::Ellipse {
            radius_x_meters: radius_x.max(0.0),
            radius_z_meters: radius_z.max(0.0),
        },
        DoodadCollisionShape::Rectangle => {
            let w = radius_x * 2.0;
            let d = radius_z * 2.0;
            if w > 0.0 && d > 0.0 {
                FootprintShape::Rectangle {
                    width_meters: w,
                    depth_meters: d,
                }
            } else {
                FootprintShape::Circle {
                    radius_meters: uniform_fallback,
                }
            }
        }
        // Baked masks are not yet loaded at runtime; fall back to scaled circle until AT4+.
        DoodadCollisionShape::Baked => FootprintShape::Circle {
            radius_meters: uniform_fallback,
        },
    }
}

fn uniform_xz(scale_xz: Vec2) -> f32 {
    if scale_xz.x > 0.0 && scale_xz.y > 0.0 {
        (scale_xz.x + scale_xz.y) * 0.5
    } else {
        scale_xz.x.max(scale_xz.y).max(0.0)
    }
}

fn effective_base_radius_x(definition: &DoodadDefinition) -> f32 {
    if definition.base_collision_radius_x_meters > 0.0 {
        definition.base_collision_radius_x_meters
    } else {
        definition.block_radius_meters
    }
}

fn effective_base_radius_z(definition: &DoodadDefinition) -> f32 {
    if definition.base_collision_radius_z_meters > 0.0 {
        definition.base_collision_radius_z_meters
    } else {
        definition.block_radius_meters
    }
}

/// Lookup collision for a record using catalog fallback.
pub fn resolve_doodad_collision_from_catalog(
    record: &DoodadRecord,
    catalog: &DoodadCatalog,
) -> DoodadInstanceCollision {
    if let Some(definition) = catalog.get(&record.definition_id) {
        resolve_doodad_collision(record, definition)
    } else {
        DoodadInstanceCollision {
            blocks_movement: crate::world::default_blocks_movement(record.kind),
            shape: FootprintShape::Circle {
                radius_meters: crate::world::occupancy::conservative_block_radius_for_kind(
                    record.kind,
                ),
            },
            yaw_radians: record.placement.collision_yaw_radians(),
        }
    }
}

/// Pitch/roll tilt diagnostic for blocking doodads.
pub fn tilted_blocker_projection_warning(record: &DoodadRecord) -> Option<String> {
    const TILT_WARN_DEG: f32 = 35.0;
    let pitch = record.placement.orientation.pitch_degrees().abs();
    let roll = record.placement.orientation.roll_degrees().abs();
    if pitch > TILT_WARN_DEG || roll > TILT_WARN_DEG {
        Some(format!(
            "TiltedBlockerProjection: pitch={pitch:.1}° roll={roll:.1}° — ground collision uses yaw + X/Z scale only"
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::asset_sizing::AssetSizingDefinition;
    use crate::world::authoring_transform::AuthoringScale;
    use crate::world::{
        ChunkCoord, DoodadDefinition, DoodadDefinitionId, DoodadKind, DoodadPlacement,
        DoodadRecord, DoodadRenderKey, DoodadSource, LocalPosition, WorldPosition,
    };

    fn sample_def() -> DoodadDefinition {
        let mut def = DoodadDefinition::new(
            DoodadDefinitionId::new("rock"),
            DoodadKind::Rock,
            "Rock",
            1.0,
            0.5,
            2.0,
            None,
            None,
            None,
            true,
            DoodadRenderKey::reserved("rock"),
        );
        def.blocks_movement = true;
        def.block_radius_meters = 1.0;
        def.base_collision_radius_x_meters = 2.0;
        def.base_collision_radius_z_meters = 0.5;
        def.collision_shape = DoodadCollisionShape::Ellipse;
        def.asset_sizing = AssetSizingDefinition::default();
        def
    }

    #[test]
    fn circle_when_radii_equal() {
        let mut def = sample_def();
        def.base_collision_radius_x_meters = 1.0;
        def.base_collision_radius_z_meters = 1.0;
        def.collision_shape = DoodadCollisionShape::Circle;
        let record = DoodadRecord::new(
            crate::world::DoodadId::new(1),
            def.id.clone(),
            def.kind,
            DoodadPlacement::identity_at(WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::ZERO),
            )),
            DoodadSource::Authored,
        );
        let collision = resolve_doodad_collision(&record, &def);
        assert!(matches!(collision.shape, FootprintShape::Circle { .. }));
    }

    #[test]
    fn ellipse_when_radii_differ() {
        let def = sample_def();
        let mut placement = DoodadPlacement::identity_at(WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::ZERO),
        ));
        placement.scale = AuthoringScale::from_non_uniform_f32(1.0, 1.0, 1.0).unwrap();
        let record = DoodadRecord::new(
            crate::world::DoodadId::new(1),
            def.id.clone(),
            def.kind,
            placement,
            DoodadSource::Authored,
        );
        let collision = resolve_doodad_collision(&record, &def);
        assert!(matches!(
            collision.shape,
            FootprintShape::Ellipse {
                radius_x_meters,
                radius_z_meters,
            } if radius_x_meters > radius_z_meters
        ));
    }

    #[test]
    fn none_shape_scales_with_baseline_and_instance() {
        let mut def = sample_def();
        def.collision_shape = DoodadCollisionShape::None;
        def.block_radius_meters = 1.0;
        def.base_collision_radius_x_meters = 1.0;
        def.base_collision_radius_z_meters = 1.0;
        def.asset_sizing.calculated_baseline_scale =
            Some(AuthoringScale::from_uniform_f32(2.0).unwrap());
        let mut placement = DoodadPlacement::identity_at(WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::ZERO),
        ));
        placement.scale = AuthoringScale::from_uniform_f32(1.5).unwrap();
        let record = DoodadRecord::new(
            crate::world::DoodadId::new(1),
            def.id.clone(),
            def.kind,
            placement,
            DoodadSource::Authored,
        );
        let collision = resolve_doodad_collision(&record, &def);
        match collision.shape {
            FootprintShape::Circle { radius_meters } => {
                assert!((radius_meters - 3.0).abs() < 0.01, "got {radius_meters}");
            }
            other => panic!("expected scaled circle, got {other:?}"),
        }
    }
}
