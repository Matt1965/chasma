//! Capture gating: portraits commit only when presentation matches authoritative state.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::mesh::morph::MeshMorphWeights;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;

use crate::units::appearance_presentation::sync::{collect_morph_primitives, morph_digest};
use crate::units::equipment_presentation::{
    UnitEquipmentMorphConfig, UnitEquipmentMorphFingerprint, UnitEquipmentSkinnedPending,
    UnitEquipmentVisual, desired_equipment_presentations_for_unit,
};
use crate::units::equipment_presentation::DesiredEquipmentPresentation;
use crate::units::UnitAppearanceMorphFingerprint;
use crate::units::presentation::UnitPresentationAppearance;
use crate::world::AppearanceParamId;
use crate::world::{
    AppearanceProfileCatalog, EquipmentVisualCatalog, ItemCatalog, UnitCatalog, UnitDefinition,
    UnitRecord, WorldData, effective_unit_render_key_str,
};

use super::cache::PortraitAppearanceSignature;
use super::equipment::UnitPortraitEquipmentIndex;
use super::signature::{portrait_signature_for_unit, portrait_slot_affects_signature};

/// Portrait-visible equipment resolved the same way as live units, filtered to signature slots.
pub fn desired_portrait_equipment_presentations(
    world: &WorldData,
    items: &ItemCatalog,
    visuals: &EquipmentVisualCatalog,
    unit_render_key: &str,
    unit: &UnitRecord,
) -> Vec<DesiredEquipmentPresentation> {
    desired_equipment_presentations_for_unit(world, items, visuals, unit_render_key, unit)
        .into_iter()
        .filter(|entry| portrait_slot_affects_signature(entry.slot))
        .collect()
}

/// True when spawned portrait equipment matches authoritative inventories and skinning finished.
pub fn portrait_actor_equipment_ready(
    actor: Entity,
    desired: &[DesiredEquipmentPresentation],
    index: &UnitPortraitEquipmentIndex,
    visuals_alive: &Query<Entity, With<UnitEquipmentVisual>>,
    pending_skinned: &Query<Entity, With<UnitEquipmentSkinnedPending>>,
    presentation: Option<&UnitPresentationAppearance>,
    equipment_morph_config: &Query<&UnitEquipmentMorphConfig>,
    equipment_morph_fingerprint: &Query<&UnitEquipmentMorphFingerprint>,
    children: &Query<&Children>,
    mesh3d: &Query<&Mesh3d>,
    skinned: &Query<&SkinnedMesh>,
) -> bool {
    for entry in desired {
        let key = (actor, entry.slot);
        if index.instance_ids.get(&key) != Some(&entry.item_instance_id) {
            return false;
        }
        let Some(visual) = index.visuals.get(&key) else {
            return false;
        };
        if visuals_alive.get(*visual).is_err() {
            return false;
        }
        if descendant_has_component(*visual, pending_skinned, children)
            || descendant_has_component(actor, pending_skinned, children)
        {
            return false;
        }
        if count_drawables_under(*visual, children, mesh3d, skinned) == 0 {
            return false;
        }
        if let Ok(config) = equipment_morph_config.get(*visual) {
            if !config.consumed_morph_params.is_empty() {
                let Some(presentation) = presentation else {
                    return false;
                };
                let expected = UnitEquipmentMorphFingerprint::from_appearance(
                    presentation.appearance.profile_id.as_str(),
                    presentation.appearance.body_variant_id.as_str(),
                    morph_digest(&presentation.appearance),
                    consumed_param_digest(&config.consumed_morph_params),
                );
                if equipment_morph_fingerprint.get(*visual) != Ok(&expected) {
                    return false;
                }
            }
        }
    }
    for (root, slot) in index.visuals.keys() {
        if *root != actor || !portrait_slot_affects_signature(*slot) {
            continue;
        }
        if !desired.iter().any(|entry| entry.slot == *slot) {
            return false;
        }
    }
    true
}

/// True when body morph weights on the portrait actor match authoritative appearance.
pub fn portrait_actor_morphs_ready(
    actor: Entity,
    presentation: Option<&UnitPresentationAppearance>,
    fingerprint: Option<&UnitAppearanceMorphFingerprint>,
    meshes: &Assets<Mesh>,
    mesh_morph: &Query<&MeshMorphWeights>,
    mesh3d: &Query<&Mesh3d>,
    children: &Query<&Children>,
) -> bool {
    let Some(presentation) = presentation else {
        return true;
    };
    let appearance = &presentation.appearance;
    let primitives = collect_morph_primitives(actor, children, mesh_morph, mesh3d);
    if primitives.is_empty() {
        return true;
    }
    let expected = UnitAppearanceMorphFingerprint::from_appearance(
        appearance.profile_id.as_str(),
        appearance.body_variant_id.as_str(),
        appearance.height_scale,
        morph_digest(appearance),
    );
    fingerprint == Some(&expected)
}

pub fn portrait_capture_signature_current(
    world: &WorldData,
    unit: &UnitRecord,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    items: &ItemCatalog,
    visuals: &EquipmentVisualCatalog,
    request: &PortraitAppearanceSignature,
) -> bool {
    portrait_signature_for_unit(
        world,
        unit,
        unit_catalog,
        appearance_profiles,
        items,
        visuals,
    )
        .is_some_and(|current| current == *request)
}

pub fn portrait_capture_presentation_ready(
    actor: Entity,
    unit: &UnitRecord,
    definition: &UnitDefinition,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    items: &ItemCatalog,
    visuals: &EquipmentVisualCatalog,
    equipment_index: &UnitPortraitEquipmentIndex,
    visuals_alive: &Query<Entity, With<UnitEquipmentVisual>>,
    pending_skinned: &Query<Entity, With<UnitEquipmentSkinnedPending>>,
    equipment_morph_config: &Query<&UnitEquipmentMorphConfig>,
    equipment_morph_fingerprint: &Query<&UnitEquipmentMorphFingerprint>,
    presentation: Option<&UnitPresentationAppearance>,
    morph_fingerprint: Option<&UnitAppearanceMorphFingerprint>,
    meshes: &Assets<Mesh>,
    mesh_morph: &Query<&MeshMorphWeights>,
    mesh3d: &Query<&Mesh3d>,
    skinned: &Query<&SkinnedMesh>,
    children: &Query<&Children>,
) -> bool {
    let unit_render_key = match effective_unit_render_key_str(unit, definition, appearance_profiles)
    {
        Ok(key) => key,
        Err(_) => return false,
    };
    let desired = desired_portrait_equipment_presentations(
        world,
        items,
        visuals,
        &unit_render_key,
        unit,
    );
    portrait_actor_equipment_ready(
        actor,
        &desired,
        equipment_index,
        visuals_alive,
        pending_skinned,
        presentation,
        equipment_morph_config,
        equipment_morph_fingerprint,
        children,
        mesh3d,
        skinned,
    ) && portrait_actor_morphs_ready(
        actor,
        presentation,
        morph_fingerprint,
        meshes,
        mesh_morph,
        mesh3d,
        children,
    )
}

fn count_drawables_under(
    root: Entity,
    children: &Query<&Children>,
    mesh3d: &Query<&Mesh3d>,
    skinned: &Query<&SkinnedMesh>,
) -> u32 {
    let mut count = 0u32;
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if mesh3d.get(entity).is_ok() || skinned.get(entity).is_ok() {
            count += 1;
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
    count
}

fn consumed_param_digest(params: &[AppearanceParamId]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for param in params {
        param.as_str().hash(&mut hasher);
    }
    hasher.finish()
}

fn descendant_has_component<T: Component>(
    root: Entity,
    query: &Query<Entity, With<T>>,
    children: &Query<&Children>,
) -> bool {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if query.contains(entity) {
            return true;
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
    false
}

