//! Pure resolution helpers for dev placement model previews (testable).

use bevy::prelude::*;

use crate::dev::dev_mode::{DefinitionId, DevModeState, DevTab};
use crate::world::{
    BuildingCatalog, BuildingDefinitionId, DoodadCatalog, DoodadDefinitionId, ItemCatalog,
    ItemDefinitionId, UnitCatalog, UnitDefinitionId,
};

use super::preview::DevPlacementPreview;

/// Client-local visual kind for a catalog placement preview (presentation only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevPlacementModelVisual {
    UnitScene {
        render_key: String,
    },
    DoodadScene {
        definition_id: DoodadDefinitionId,
        render_key: String,
    },
    BuildingScene {
        definition_id: BuildingDefinitionId,
        render_key: String,
        uses_model_child: bool,
    },
    ItemScene {
        item_id: ItemDefinitionId,
        render_key: String,
    },
    ItemFallbackSphere,
}

/// Stable key — when this changes, preview entities are rebuilt.
pub fn dev_placement_model_visual_key(
    dev_state: &DevModeState,
    definition: &DefinitionId,
) -> String {
    format!(
        "{definition:?}:yaw={}:scale={}:tab={}:pile={}",
        dev_state.placement_yaw_deg,
        dev_state.placement_uniform_scale,
        dev_state.active_tab as u8,
        dev_state.inventory.pile_placement_armed,
    )
}

/// Whether model ghosts should be shown this frame.
pub fn dev_placement_model_preview_active(
    dev_state: &DevModeState,
    preview: &DevPlacementPreview,
) -> bool {
    if !dev_state.enabled || !dev_state.show_preview || !preview.active {
        return false;
    }
    dev_state.selected_definition.is_some()
}

pub fn resolve_dev_placement_model_visual(
    definition: &DefinitionId,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    item_catalog: &ItemCatalog,
) -> Option<DevPlacementModelVisual> {
    match definition {
        DefinitionId::Unit(id) => {
            let definition = unit_catalog.get(id)?;
            let render_key = definition.render_key.0.clone()?;
            Some(DevPlacementModelVisual::UnitScene { render_key })
        }
        DefinitionId::Doodad(id) => {
            let definition = doodad_catalog.get(id)?;
            let render_key = definition.render_key.0.clone()?;
            Some(DevPlacementModelVisual::DoodadScene {
                definition_id: id.clone(),
                render_key,
            })
        }
        DefinitionId::Building(id) => {
            let definition = building_catalog.get(id)?;
            let render_key = crate::buildings::ghost_render_key(definition)?;
            Some(DevPlacementModelVisual::BuildingScene {
                definition_id: id.clone(),
                render_key,
                uses_model_child: crate::world::building_uses_model_child(definition),
            })
        }
        DefinitionId::Item(id) => {
            let definition = item_catalog.get(id)?;
            if let Some(render_key) = definition.render_key.0.clone() {
                Some(DevPlacementModelVisual::ItemScene {
                    item_id: id.clone(),
                    render_key,
                })
            } else {
                Some(DevPlacementModelVisual::ItemFallbackSphere)
            }
        }
        DefinitionId::InventoryProfile(_) => None,
    }
}

/// Item catalog selections only preview on the Items tab (held cursor or armed pile placement).
pub fn item_catalog_preview_armed(dev_state: &DevModeState) -> bool {
    dev_state.active_tab == DevTab::Items
        && (dev_state.dev_held_item_id().is_some() || dev_state.inventory.pile_placement_armed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::tools::preview::PreviewPoint;
    use crate::world::{
        ChunkCoord, LocalPosition, UnitDefinition, UnitDefinitionId, UnitRenderKey,
        WeaponDefinitionId, WorldPosition,
    };

    fn wolf_unit() -> UnitDefinition {
        UnitDefinition::new_test(
            UnitDefinitionId::new("wolf"),
            "wolf",
            "Wolf",
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1.0,
            "Common",
            4.0,
            0.5,
            40.0,
            WeaponDefinitionId::new("weapon_fists"),
            true,
            UnitRenderKey::reserved("wolf"),
        )
    }

    #[test]
    fn resolves_unit_visual_from_catalog() {
        let units = UnitCatalog::from_definitions(vec![wolf_unit()]).unwrap();
        let visual = resolve_dev_placement_model_visual(
            &DefinitionId::Unit(UnitDefinitionId::new("wolf")),
            &units,
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &ItemCatalog::default(),
        );
        assert_eq!(
            visual,
            Some(DevPlacementModelVisual::UnitScene {
                render_key: "wolf".to_string(),
            })
        );
    }

    #[test]
    fn visual_key_changes_when_yaw_changes() {
        let mut state = DevModeState::default();
        state.placement_yaw_deg = 0.0;
        let def = DefinitionId::Unit(UnitDefinitionId::new("wolf"));
        let a = dev_placement_model_visual_key(&state, &def);
        state.placement_yaw_deg = 15.0;
        let b = dev_placement_model_visual_key(&state, &def);
        assert_ne!(a, b);
    }

    #[test]
    fn preview_inactive_when_dev_disabled_or_no_points() {
        let mut state = DevModeState::default();
        state.enabled = true;
        state.select_definition(DefinitionId::Unit(UnitDefinitionId::new("wolf")));
        let preview = DevPlacementPreview {
            active: true,
            points: vec![PreviewPoint {
                position: WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::ZERO),
                ),
                valid: true,
            }],
        };
        assert!(dev_placement_model_preview_active(&state, &preview));
        state.enabled = false;
        assert!(!dev_placement_model_preview_active(&state, &preview));
    }
}
