//! Apply authoritative unit appearance morph weights to spawned glTF instances.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::mesh::morph::{MeshMorphWeights, MorphWeights};
use bevy::prelude::*;

use crate::units::components::UnitSceneRoot;
use crate::units::presentation::UnitPresentationAppearance;
use crate::units::sync::UnitSyncOverrides;
use crate::world::{
    AppearanceProfileCatalog, effective_render_key_for_appearance, resolve_morph_weights,
};

use super::components::UnitAppearanceMorphFingerprint;

/// Reconcile morph weights for rendered units from authoritative appearance data.
///
/// Writes resolved weights directly to each rendered primitive's [`MeshMorphWeights`]
/// (the component the renderer extracts). Parent [`MorphWeights`] from glTF import are
/// removed so Bevy's inheritance pass cannot overwrite primitive weights with stale values.
pub fn sync_unit_appearance_morphs(
    mut commands: Commands,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    overrides: Option<Res<UnitSyncOverrides>>,
    meshes: Res<Assets<Mesh>>,
    mut mesh_morph: Query<&mut MeshMorphWeights>,
    mesh3d: Query<&Mesh3d>,
    child_of: Query<&ChildOf>,
    unit_roots: Query<(
        Entity,
        &UnitPresentationAppearance,
        &UnitSceneRoot,
        Option<&UnitAppearanceMorphFingerprint>,
    )>,
    children: Query<&Children>,
) {
    let _ = overrides;
    for (render_entity, presentation, _, fingerprint) in &unit_roots {
        let appearance = &presentation.appearance;
        let Some(profile) = appearance_profiles.get(&appearance.profile_id) else {
            continue;
        };
        let render_key = match effective_render_key_for_appearance(appearance, &appearance_profiles)
        {
            Ok(key) => key.0.clone().unwrap_or_default(),
            Err(_) => continue,
        };

        let morph_digest = morph_digest(appearance);
        let next_fingerprint = UnitAppearanceMorphFingerprint::from_appearance(
            appearance.profile_id.as_str(),
            appearance.body_variant_id.as_str(),
            appearance.height_scale,
            morph_digest,
        );
        if fingerprint == Some(&next_fingerprint) {
            continue;
        }

        let primitives = collect_morph_primitives(
            render_entity,
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
            let weights = match resolve_morph_weights(
                profile,
                &appearance.body_variant_id,
                &appearance.morphs,
                target_names,
            ) {
                Ok(weights) => weights,
                Err(error) => {
                    warn!(
                        "morph resolve failed for presentation root ({}): {error}",
                        render_key
                    );
                    all_applied = false;
                    continue;
                }
            };

            if !apply_mesh_morph_weights(&mut mesh_morph, primitive.entity, &weights) {
                all_applied = false;
                continue;
            }

            if let Ok(parent) = child_of.get(primitive.entity).map(|value| value.parent()) {
                commands.entity(parent).remove::<MorphWeights>();
            }
        }

        if all_applied {
            commands
                .entity(render_entity)
                .insert(next_fingerprint);
        }
    }
}

/// A rendered glTF primitive that owns [`MeshMorphWeights`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MorphPrimitiveTarget {
    pub entity: Entity,
    pub mesh: Handle<Mesh>,
}

/// Collect every morph-capable rendered primitive under a unit scene root.
pub(crate) fn collect_morph_primitives(
    root: Entity,
    children: &Query<&Children>,
    mesh_morph: &Query<&MeshMorphWeights>,
    mesh3d: &Query<&Mesh3d>,
) -> Vec<MorphPrimitiveTarget> {
    let mut out = Vec::new();
    for entity in descendants(root, children) {
        if !mesh_morph.contains(entity) {
            continue;
        }
        if let Ok(mesh3d) = mesh3d.get(entity) {
            out.push(MorphPrimitiveTarget {
                entity,
                mesh: mesh3d.0.clone(),
            });
        }
    }
    out
}

/// Write resolved weights to the primitive [`MeshMorphWeights`] that the renderer reads.
pub(crate) fn apply_mesh_morph_weights(
    mesh_morph: &mut Query<&mut MeshMorphWeights>,
    entity: Entity,
    weights: &[f32],
) -> bool {
    let Ok(mut component) = mesh_morph.get_mut(entity) else {
        return false;
    };
    let slice = component.weights_mut();
    if slice.len() != weights.len() {
        return false;
    }
    slice.copy_from_slice(weights);
    true
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

fn descendants(root: Entity, children: &Query<&Children>) -> Vec<Entity> {
    let mut stack = vec![root];
    let mut out = Vec::new();
    while let Some(entity) = stack.pop() {
        out.push(entity);
        if let Ok(kids) = children.get(entity) {
            for child in kids.iter() {
                stack.push(child);
            }
        }
    }
    out
}
