//! Build Mode terrain overlay auto-selection (ADR-104 TF4).

use bevy::prelude::*;

use crate::terrain::field_overlay::TerrainOverlayState;
use crate::world::{BuildingFieldRequirementCatalog, TerrainFieldCatalog, TerrainFieldId};

use super::state::BuildModeState;

/// Apply or clear temporary terrain overlay while placing field-dependent buildings.
pub fn sync_build_mode_terrain_overlay(
    build_mode: Res<BuildModeState>,
    mut overlay: ResMut<TerrainOverlayState>,
    requirements: Res<BuildingFieldRequirementCatalog>,
    field_catalog: Res<TerrainFieldCatalog>,
) {
    let desired = build_mode
        .ghost_definition_id()
        .and_then(|definition_id| requirements.primary_overlay_field(definition_id))
        .filter(|field_id| {
            field_catalog
                .get(field_id)
                .is_some_and(|field| field.enabled)
        });

    let current = overlay.selection.temporary_override.clone();
    if current == desired {
        return;
    }

    if let Some(field_id) = desired {
        overlay.set_temporary_override(Some(field_id));
    } else {
        overlay.clear_temporary_override();
    }
}

/// Clear temporary overlay when Build Mode ends.
pub fn clear_build_mode_terrain_overlay_on_exit(
    build_mode: Res<BuildModeState>,
    mut overlay: ResMut<TerrainOverlayState>,
) {
    if build_mode.is_active() {
        return;
    }
    if overlay.selection.temporary_override.is_some() {
        overlay.clear_temporary_override();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::BuildingDefinitionId;

    #[test]
    fn temporary_override_preserves_manual_field() {
        let mut overlay = TerrainOverlayState::default();
        overlay.set_manual_field(Some(TerrainFieldId::new("copper")));
        overlay.set_temporary_override(Some(TerrainFieldId::new("iron")));
        assert_eq!(
            overlay.selection.manual.as_ref().map(|id| id.as_str()),
            Some("copper")
        );
        assert_eq!(
            overlay.effective_field().map(|id| id.as_str()),
            Some("iron")
        );
        overlay.clear_temporary_override();
        assert_eq!(
            overlay.effective_field().map(|id| id.as_str()),
            Some("copper")
        );
    }

    #[test]
    fn building_without_requirements_clears_override() {
        let requirements = BuildingFieldRequirementCatalog::default();
        assert!(
            requirements
                .primary_overlay_field(&BuildingDefinitionId::new("hut"))
                .is_none()
        );
    }
}
