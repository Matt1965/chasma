//! CG5 armor morph sync onto skinned equipment overlays.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::mesh::morph::{MeshMorphWeights, MorphWeights};
use bevy::prelude::*;

use crate::units::appearance_presentation::sync::{
    apply_mesh_morph_weights, collect_morph_primitives,
};
use crate::units::components::UnitSceneRoot;
use crate::units::presentation::UnitPresentationAppearance;
use crate::units::sync::UnitSyncOverrides;
use crate::world::{AppearanceProfileCatalog, resolve_equipment_morph_weights};

use super::components::{
    UnitEquipmentMorphConfig, UnitEquipmentMorphFingerprint, UnitEquipmentSceneRoot,
    UnitEquipmentSkinnedPending, UnitEquipmentVisual,
};

/// Reconcile morph weights for skinned equipment overlays from unit appearance.
pub fn sync_unit_equipment_morphs(
    mut commands: Commands,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    overrides: Option<Res<UnitSyncOverrides>>,
    meshes: Res<Assets<Mesh>>,
    mut mesh_morph: Query<&mut MeshMorphWeights>,
    mesh3d: Query<&Mesh3d>,
    child_of: Query<&ChildOf>,
    unit_roots: Query<&UnitPresentationAppearance, With<UnitSceneRoot>>,
    equipment: Query<
        (
            Entity,
            &UnitEquipmentVisual,
            &UnitEquipmentMorphConfig,
            &ChildOf,
            Option<&UnitEquipmentMorphFingerprint>,
        ),
        (
            With<UnitEquipmentSceneRoot>,
            Without<UnitEquipmentSkinnedPending>,
        ),
    >,
    children: Query<&Children>,
) {
    let _ = overrides;
    for (visual_entity, _, morph_config, parent, fingerprint) in &equipment {
        if morph_config.consumed_morph_params.is_empty() {
            continue;
        }
        let unit_root = parent.parent();
        let Ok(presentation) = unit_roots.get(unit_root) else {
            continue;
        };
        let appearance = &presentation.appearance;
        let Some(profile) = appearance_profiles.get(&appearance.profile_id) else {
            continue;
        };

        let morph_digest = morph_digest(appearance);
        let consumed_digest = consumed_param_digest(&morph_config.consumed_morph_params);
        let next_fingerprint = UnitEquipmentMorphFingerprint::from_appearance(
            appearance.profile_id.as_str(),
            appearance.body_variant_id.as_str(),
            morph_digest,
            consumed_digest,
        );
        if fingerprint == Some(&next_fingerprint) {
            continue;
        }

        let primitives = collect_morph_primitives(
            visual_entity,
            &children,
            &mesh_morph.as_readonly(),
            &mesh3d,
        );
        if primitives.is_empty() {
            continue;
        }

        let mut all_applied = true;
        for primitive in primitives {
            let Some(mesh) = meshes.get(&primitive.mesh) else {
                all_applied = false;
                continue;
            };
            let Some(target_names) = mesh.morph_target_names() else {
                all_applied = false;
                continue;
            };
            let weights = match resolve_equipment_morph_weights(
                profile,
                &appearance.body_variant_id,
                &appearance.morphs,
                target_names,
                &morph_config.consumed_morph_params,
            ) {
                Ok(weights) => weights,
                Err(error) => {
                    warn!(
                        "equipment morph resolve failed for unit presentation: {error}"
                    );
                    all_applied = false;
                    continue;
                }
            };

            if !apply_mesh_morph_weights(&mut mesh_morph, primitive.entity, &weights) {
                all_applied = false;
                continue;
            }

            if let Ok(parent_entity) = child_of.get(primitive.entity).map(|value| value.parent()) {
                commands.entity(parent_entity).remove::<MorphWeights>();
            }
        }

        if all_applied {
            commands.entity(visual_entity).insert(next_fingerprint);
        }
    }
}

fn morph_digest(appearance: &crate::world::UnitAppearance) -> u64 {
    let mut hasher = DefaultHasher::new();
    appearance.profile_id.as_str().hash(&mut hasher);
    appearance.body_variant_id.as_str().hash(&mut hasher);
    for (key, value) in &appearance.morphs {
        key.as_str().hash(&mut hasher);
        value.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

fn consumed_param_digest(params: &[crate::world::AppearanceParamId]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for param in params {
        param.as_str().hash(&mut hasher);
    }
    hasher.finish()
}
