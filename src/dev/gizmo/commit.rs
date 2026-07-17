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
