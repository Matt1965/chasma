//! Contextual command layer — intent enrichment between client intents and unit orders (ADR-041 U-UI5).
//!
//! Pipeline: `ClientIntent` → context resolve → command build → [`issue_unit_order`].

mod command_builder;
mod command_palette;
mod command_types;
mod context_resolver;

pub use command_builder::{
    build_command_plan, unit_orders_for_plan, BuiltCommandPlan, CommandBuildError,
};
#[cfg(test)]
pub use command_builder::build_command_plan_or_fallback_move;
pub use command_palette::{
    available_commands_for_selection, unit_supports_command, CommandPaletteEntry,
};
pub use command_types::{CommandTarget, CommandType, ContextualCommandIntent};
pub use context_resolver::{resolve_contextual_command, resolve_palette_command, CommandResolutionContext};

use bevy::prelude::*;

/// Read-only hook for gameplay UI — last resolved command type and tooltip.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct ResolvedCommandFeedback {
    pub command_type: Option<CommandType>,
    pub tooltip: Option<String>,
}

impl ResolvedCommandFeedback {
    pub fn set_resolved(&mut self, command_type: CommandType) {
        self.command_type = Some(command_type);
        self.tooltip = Some(command_type.tooltip().to_string());
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition, WorldPosition};
    use bevy::prelude::Vec3;

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn resolved_feedback_reflects_command_type_without_simulation() {
        let mut feedback = ResolvedCommandFeedback::default();
        feedback.set_resolved(CommandType::Move);
        assert_eq!(feedback.command_type, Some(CommandType::Move));
        assert!(feedback.tooltip.as_ref().is_some_and(|t| t.contains("Move")));
        feedback.clear();
        assert!(feedback.command_type.is_none());
    }

    #[test]
    fn end_to_end_classify_and_build_terrain_move() {
        use crate::units::input::SelectedUnits;

        let units = [crate::world::UnitId::new(1)];
        let target = CommandTarget::Terrain { position: pos(8.0, 8.0) };
        let world = crate::world::WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let unit_catalog = crate::world::UnitCatalog::default();
        let weapon_catalog = crate::world::WeaponCatalog::default();
        let ctx = CommandResolutionContext {
            selected_units: &units,
            target: target.clone(),
            world: &world,
            unit_catalog: &unit_catalog,
            weapon_catalog: &weapon_catalog,
            targeting_policy: crate::world::AttackTargetingPolicy::default(),
        };
        let intent = resolve_contextual_command(&ctx).unwrap();
        let mut selection = SelectedUnits::default();
        selection.set_single(units[0]);
        let plan = build_command_plan(&intent, &selection, &world).unwrap();
        assert!(matches!(plan, BuiltCommandPlan::MoveTo { .. }));
    }
}
