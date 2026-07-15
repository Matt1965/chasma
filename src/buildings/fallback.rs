//! Diagnostic fallback presentation when building GLB assets are unavailable (ADR-095 BA1).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::{Affiliation, BuildingDefinition, BuildingLifecycleState, BuildingRecord};

use super::placeholder::{diagnostic_fallback_color, placeholder_mesh_size};

/// Why a building uses the diagnostic fallback mesh instead of its GLB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum BuildingFallbackReason {
    MissingDefinition,
    MissingRenderKey,
    AssetLoadFailed,
    SceneNotReady,
}

impl BuildingFallbackReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::MissingDefinition => "missing definition",
            Self::MissingRenderKey => "missing render key",
            Self::AssetLoadFailed => "asset load failed",
            Self::SceneNotReady => "scene not ready",
        }
    }
}

/// Cached diagnostic fallback meshes (magenta/error cuboids sized by footprint).
#[derive(Resource, Default)]
pub struct BuildingFallbackAssets {
    meshes: HashMap<[u32; 3], Handle<Mesh>>,
    materials: HashMap<(BuildingLifecycleState, Affiliation), Handle<StandardMaterial>>,
}

impl BuildingFallbackAssets {
    pub fn mesh_for_definition(
        &mut self,
        meshes: &mut Assets<Mesh>,
        definition: &BuildingDefinition,
    ) -> Handle<Mesh> {
        let size = placeholder_mesh_size(definition);
        let key = [
            (size.x * 100.0).round() as u32,
            (size.y * 100.0).round() as u32,
            (size.z * 100.0).round() as u32,
        ];
        self.meshes
            .entry(key)
            .or_insert_with(|| meshes.add(Cuboid::new(size.x, size.y, size.z)))
            .clone()
    }

    pub fn material_for_state(
        &mut self,
        materials: &mut Assets<StandardMaterial>,
        lifecycle: BuildingLifecycleState,
        affiliation: Affiliation,
    ) -> Handle<StandardMaterial> {
        self.materials
            .entry((lifecycle, affiliation))
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: diagnostic_fallback_color(lifecycle, affiliation),
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                })
            })
            .clone()
    }
}

/// Spawn a visibly diagnostic cuboid (never used for valid configured assets).
pub fn spawn_diagnostic_fallback_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    fallback_assets: &mut BuildingFallbackAssets,
    record: &BuildingRecord,
    definition: &BuildingDefinition,
    marker: super::components::BuildingRenderEntity,
    translation: Vec3,
    reason: BuildingFallbackReason,
) -> Entity {
    if reason != BuildingFallbackReason::SceneNotReady {
        warn!(
            "building {} (`{}`) using diagnostic fallback: {}",
            record.id.raw(),
            record.definition_id.as_str(),
            reason.label()
        );
    }

    let mesh = fallback_assets.mesh_for_definition(meshes, definition);
    let material = fallback_assets.material_for_state(
        materials,
        record.lifecycle_state,
        record.ownership.affiliation,
    );

    commands
        .spawn((
            marker,
            super::components::BuildingDiagnosticFallback { reason },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform {
                translation,
                rotation: record.placement.rotation,
                scale: Vec3::ONE,
            },
            Visibility::default(),
        ))
        .id()
}
