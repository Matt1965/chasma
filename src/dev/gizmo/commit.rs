//! Commit preview transforms through authoritative APIs (ADR-099, ADR-100 DT4).

use super::handles::policy_for_target;
use super::state::TransformEditState;
use super::state::{DoodadPreviewPlacement, building_uniform_scale_from_preview};
use super::tool::SelectedWorldObject;
use crate::world::{
    BuildingTerrainAssessmentStore, BuildingTransformCandidate, BuildingTransformCatalogs,
    BuildingTransformEditError, BuildingTransformEditOptions, DoodadCatalog,
    DoodadTransformCandidate, DoodadTransformEditOptions, FootprintCatalog, InteriorProfileCatalog,
    OccupancyCatalogs, TransformEditError, UnitCatalog, update_building_transform,
    update_doodad_transform,
};

pub fn commit_doodad_preview(
    world: &mut crate::world::WorldData,
    catalog: &DoodadCatalog,
    building_catalog: &crate::world::BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    doodad_id: crate::world::DoodadId,
    preview: DoodadPreviewPlacement,
    options: DoodadTransformEditOptions,
) -> Result<(), TransformEditError> {
    let occ = OccupancyCatalogs {
        doodad: catalog,
        building: building_catalog,
        footprint: footprint_catalog,
    };
    update_doodad_transform(
        world,
        catalog,
        doodad_id,
        DoodadTransformCandidate {
            position: preview.position,
            orientation: preview.orientation,
            scale: preview.scale,
        },
        options,
        Some(occ),
    )?;
    Ok(())
}

pub fn commit_building_preview(
    world: &mut crate::world::WorldData,
    building_catalog: &crate::world::BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    doodad_catalog: &DoodadCatalog,
    interior_catalog: &InteriorProfileCatalog,
    unit_catalog: &UnitCatalog,
    building_id: crate::world::BuildingId,
    preview: DoodadPreviewPlacement,
    options: BuildingTransformEditOptions,
) -> Result<(), BuildingTransformEditError> {
    update_building_transform(
        world,
        BuildingTransformCatalogs {
            building: building_catalog,
            footprint: footprint_catalog,
            doodad: doodad_catalog,
            interior: interior_catalog,
            unit: unit_catalog,
        },
        building_id,
        BuildingTransformCandidate {
            position: preview.position,
            orientation: preview.orientation,
            uniform_scale: building_uniform_scale_from_preview(preview),
        },
        options,
    )?;
    Ok(())
}

/// Whether the live preview differs from authoritative world placement.
pub fn preview_differs_from_authoritative(
    world: &crate::world::WorldData,
    target: SelectedWorldObject,
    preview: DoodadPreviewPlacement,
) -> bool {
    match target {
        SelectedWorldObject::Doodad(id) => {
            let Some(record) = world.get_doodad(id) else {
                return false;
            };
            let placement = record.placement;
            preview.position != placement.position
                || preview.orientation != placement.orientation
                || preview.scale != placement.scale
        }
        SelectedWorldObject::Building(id) => {
            let Some(record) = world.get_building(id) else {
                return false;
            };
            let placement = record.placement;
            let preview_yaw = preview.orientation.yaw_degrees();
            let authored_yaw = crate::world::QuantizedOrientation::from_quat(placement.rotation)
                .map(|orientation| orientation.yaw_degrees())
                .unwrap_or(0.0);
            preview.position != placement.position
                || (preview_yaw - authored_yaw).abs() > 0.05
                || (building_uniform_scale_from_preview(preview).to_f32()
                    - placement.uniform_scale_f32())
                    .abs()
                    > 0.001
        }
        SelectedWorldObject::ItemPile(_) => false,
    }
}

/// Dev gizmo commit options — bypass strict placement gates that fight interactive edits.
pub fn dev_gizmo_doodad_commit_options(
    keyboard: &bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>,
) -> DoodadTransformEditOptions {
    DoodadTransformEditOptions {
        allow_overlap: keyboard.pressed(bevy::input::keyboard::KeyCode::KeyO),
        follow_ground: keyboard.pressed(bevy::input::keyboard::KeyCode::KeyG),
        bypass_placement_validation: true,
        bypass_definition_scale_range: true,
    }
}

pub fn dev_gizmo_building_commit_options(
    keyboard: &bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>,
) -> BuildingTransformEditOptions {
    BuildingTransformEditOptions {
        allow_overlap: keyboard.pressed(bevy::input::keyboard::KeyCode::KeyO),
        follow_ground: keyboard.pressed(bevy::input::keyboard::KeyCode::KeyG),
        bypass_placement_validation: true,
        cancel_dependencies: keyboard.pressed(bevy::input::keyboard::KeyCode::KeyC),
        allow_instance_scale_override: true,
    }
}

pub fn try_commit_edit(
    edit: &mut TransformEditState,
    world: &mut crate::world::WorldData,
    catalog: &DoodadCatalog,
    building_catalog: &crate::world::BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interior_catalog: &InteriorProfileCatalog,
    unit_catalog: &UnitCatalog,
    doodad_options: DoodadTransformEditOptions,
    building_options: BuildingTransformEditOptions,
    assessment_store: Option<&mut BuildingTerrainAssessmentStore>,
) -> bool {
    let Some(target) = edit.target else {
        return false;
    };
    let policy = policy_for_target(target, building_catalog, world);
    if !policy.can_commit {
        edit.last_error = policy
            .commit_blocked_reason
            .unwrap_or("commit not supported")
            .to_string();
        edit.preview_valid = false;
        return false;
    }
    let Some(preview) = edit.preview_placement else {
        return false;
    };
    match target {
        SelectedWorldObject::Doodad(doodad_id) => {
            match commit_doodad_preview(
                world,
                catalog,
                building_catalog,
                footprint_catalog,
                doodad_id,
                preview,
                doodad_options,
            ) {
                Ok(()) => {
                    edit.last_error.clear();
                    edit.preview_valid = true;
                    true
                }
                Err(err) => {
                    edit.last_error = format!("{err:?}");
                    edit.preview_valid = false;
                    false
                }
            }
        }
        SelectedWorldObject::Building(building_id) => {
            match commit_building_preview(
                world,
                building_catalog,
                footprint_catalog,
                catalog,
                interior_catalog,
                unit_catalog,
                building_id,
                preview,
                building_options,
            ) {
                Ok(()) => {
                    if let Some(store) = assessment_store {
                        store.mark_dirty(building_id);
                    }
                    edit.last_error.clear();
                    edit.preview_valid = true;
                    true
                }
                Err(err) => {
                    edit.last_error = format!("{err:?}");
                    edit.preview_valid = false;
                    false
                }
            }
        }
        SelectedWorldObject::ItemPile(_) => false,
    }
}
