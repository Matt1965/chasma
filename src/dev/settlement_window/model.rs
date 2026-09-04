//! Pure settlement Dev UI model (camera context + world reads).

use crate::client::CameraSettlementContext;
use crate::world::{SettlementId, UnitId, WorldData, assign_unit_settlement};

/// Read-only settlement summary for Dev UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementDevSummary {
    pub focused: bool,
    pub display_name: String,
    pub unit_count: usize,
    pub building_count: usize,
    pub ai_enabled: bool,
}

impl Default for SettlementDevSummary {
    fn default() -> Self {
        Self {
            focused: false,
            display_name: "No focused settlement".into(),
            unit_count: 0,
            building_count: 0,
            ai_enabled: false,
        }
    }
}

/// Build summary from camera context and authoritative world data.
pub fn build_settlement_dev_summary(
    world: &WorldData,
    context: &CameraSettlementContext,
) -> SettlementDevSummary {
    let Some(settlement_id) = context.focused_settlement_id else {
        return SettlementDevSummary::default();
    };
    let Some(record) = world.settlement_store().get_settlement(settlement_id) else {
        return SettlementDevSummary::default();
    };
    let unit_count = world
        .settlement_store()
        .units_for_settlement(settlement_id)
        .len();
    let building_count = world
        .settlement_store()
        .buildings_for_settlement(settlement_id)
        .len();
    let ai_enabled = world
        .settlement_state_store()
        .get(settlement_id)
        .map(|state| state.policies.automation_enabled)
        .unwrap_or(false);
    SettlementDevSummary {
        focused: true,
        display_name: record.display_name.clone(),
        unit_count,
        building_count,
        ai_enabled,
    }
}

pub fn format_focused_line(summary: &SettlementDevSummary) -> String {
    if summary.focused {
        format!("Focused: {}", summary.display_name)
    } else {
        "Focused: No focused settlement".into()
    }
}

pub fn format_ai_line(summary: &SettlementDevSummary) -> String {
    if !summary.focused {
        "AI: —".into()
    } else if summary.ai_enabled {
        "AI: Enabled".into()
    } else {
        "AI: Disabled".into()
    }
}

/// Assign gameplay-selected units to an explicit settlement (membership authority).
pub fn assign_selected_units_to_settlement(
    world: &mut WorldData,
    unit_ids: &[UnitId],
    settlement_id: SettlementId,
) -> Result<usize, crate::world::SettlementMembershipError> {
    let mut assigned = 0usize;
    for &unit_id in unit_ids {
        assign_unit_settlement(world, unit_id, Some(settlement_id))?;
        assigned += 1;
    }
    Ok(assigned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, ChunkCoord, ChunkData, ChunkLayout, Heightfield, LocalPosition,
        SettlementKind, SettlementOwnership, WorldPosition, create_settlement,
    };
    use bevy::prelude::Vec3;

    fn test_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn summary_uses_camera_context_settlement_id() {
        let mut world = test_world();
        let report = create_settlement(
            &mut world,
            pos(64.0, 64.0),
            "New Haven",
            SettlementOwnership::player_default(),
            SettlementKind::Town,
            Some(48.0),
            None,
            0,
        )
        .unwrap();
        let context = CameraSettlementContext {
            focused_settlement_id: Some(report.settlement_id),
            focus_world_position: Some(pos(64.0, 64.0)),
        };
        let summary = build_settlement_dev_summary(&world, &context);
        assert!(summary.focused);
        assert_eq!(summary.display_name, "New Haven");
    }

    #[test]
    fn none_context_yields_safe_empty_summary() {
        let world = test_world();
        let summary = build_settlement_dev_summary(&world, &CameraSettlementContext::default());
        assert!(!summary.focused);
        assert_eq!(
            format_focused_line(&summary),
            "Focused: No focused settlement"
        );
        assert_eq!(format_ai_line(&summary), "AI: —");
    }

    #[test]
    fn summary_reflects_member_counts() {
        use crate::world::{
            InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog,
            UnitCatalog, UnitOwnership, UnitSource, create_unit_with_inventory,
            starter_inventory_profile_definitions, starter_item_category_definitions,
            starter_item_definitions, starter_unit_definitions,
        };

        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();

        let mut world = test_world();
        let report = create_settlement(
            &mut world,
            pos(64.0, 64.0),
            "Harbor",
            SettlementOwnership::player_default(),
            SettlementKind::Town,
            Some(48.0),
            None,
            0,
        )
        .unwrap();
        let unit = create_unit_with_inventory(
            &unit_catalog,
            &mut world,
            &crate::world::UnitDefinitionId::new("bandit"),
            pos(66.0, 66.0),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            &ctx,
        )
        .unwrap();
        assign_selected_units_to_settlement(&mut world, &[unit.id], report.settlement_id).unwrap();

        let context = CameraSettlementContext {
            focused_settlement_id: Some(report.settlement_id),
            focus_world_position: None,
        };
        let summary = build_settlement_dev_summary(&world, &context);
        assert_eq!(summary.unit_count, 1);
        assert_eq!(summary.building_count, 0);
    }

    #[test]
    fn camera_context_switch_changes_summary_settlement() {
        let mut world = test_world();
        let a = create_settlement(
            &mut world,
            pos(64.0, 64.0),
            "Alpha",
            SettlementOwnership::player_default(),
            SettlementKind::Town,
            Some(48.0),
            None,
            0,
        )
        .unwrap();
        let b = create_settlement(
            &mut world,
            pos(200.0, 64.0),
            "Bravo",
            SettlementOwnership::player_default(),
            SettlementKind::Town,
            Some(48.0),
            None,
            0,
        )
        .unwrap();
        let summary_a = build_settlement_dev_summary(
            &world,
            &CameraSettlementContext {
                focused_settlement_id: Some(a.settlement_id),
                focus_world_position: None,
            },
        );
        let summary_b = build_settlement_dev_summary(
            &world,
            &CameraSettlementContext {
                focused_settlement_id: Some(b.settlement_id),
                focus_world_position: None,
            },
        );
        assert_eq!(summary_a.display_name, "Alpha");
        assert_eq!(summary_b.display_name, "Bravo");
    }
}
