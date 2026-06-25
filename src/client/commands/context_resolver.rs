//! Context resolution — classifies clicks into [`ContextualCommandIntent`] (ADR-041 U-UI5, ADR-056 C3).

use bevy::prelude::Vec3;

use crate::world::{is_valid_attack_target, AttackTargetingPolicy, UnitCatalog, WeaponCatalog, WorldData};

use crate::world::{UnitId, WorldPosition};

use super::command_types::{CommandTarget, CommandType, ContextualCommandIntent};

/// Inputs available when resolving a right-click into a contextual command.
#[derive(Debug, Clone)]
pub struct CommandResolutionContext<'a> {
    pub selected_units: &'a [UnitId],
    pub target: CommandTarget,
    pub world: &'a WorldData,
    pub unit_catalog: &'a UnitCatalog,
    pub weapon_catalog: &'a WeaponCatalog,
    pub targeting_policy: AttackTargetingPolicy,
}

/// Classify a command target given the current selection.
///
/// Returns `None` when the click cannot produce a command (empty selection).
pub fn resolve_contextual_command(
    ctx: &CommandResolutionContext<'_>,
) -> Option<ContextualCommandIntent> {
    if ctx.selected_units.is_empty() {
        return None;
    }

    match &ctx.target {
        CommandTarget::Terrain { position } => Some(ContextualCommandIntent {
            command_type: CommandType::Move,
            target: CommandTarget::Terrain { position: *position },
        }),
        CommandTarget::Unit { unit_id } => {
            let attacker = *ctx.selected_units.first()?;
            if is_valid_attack_target(
                ctx.world,
                attacker,
                *unit_id,
                ctx.weapon_catalog,
                ctx.unit_catalog,
                ctx.targeting_policy,
            ) || any_selected_can_attack(ctx, *unit_id)
            {
                Some(ContextualCommandIntent {
                    command_type: CommandType::Attack,
                    target: CommandTarget::Unit { unit_id: *unit_id },
                })
            } else {
                Some(ContextualCommandIntent {
                    command_type: CommandType::Move,
                    target: CommandTarget::Unit { unit_id: *unit_id },
                })
            }
        }
    }
}

fn any_selected_can_attack(ctx: &CommandResolutionContext<'_>, target: UnitId) -> bool {
    ctx.selected_units.iter().any(|attacker| {
        is_valid_attack_target(
            ctx.world,
            *attacker,
            target,
            ctx.weapon_catalog,
            ctx.unit_catalog,
            ctx.targeting_policy,
        )
    })
}

/// Resolve an explicit palette command (keyboard/UI hotkey hook).
pub fn resolve_palette_command(
    command_type: CommandType,
    selected_units: &[UnitId],
    target: Option<CommandTarget>,
) -> Option<ContextualCommandIntent> {
    if selected_units.is_empty() {
        return None;
    }

    let target = target.unwrap_or(CommandTarget::Terrain {
        position: WorldPosition::new(
            crate::world::ChunkCoord::new(0, 0),
            crate::world::LocalPosition::new(Vec3::ZERO),
        ),
    });

    Some(ContextualCommandIntent { command_type, target })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        create_unit_with_ownership, ChunkCoord, ChunkLayout, LocalPosition, UnitDefinitionId,
        UnitOwnership, UnitSource, WorldData, WorldPosition,
    };
    use bevy::prelude::Vec3;

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn ctx<'a>(
        units: &'a [UnitId],
        target: CommandTarget,
        world: &'a WorldData,
        unit_catalog: &'a UnitCatalog,
        weapon_catalog: &'a WeaponCatalog,
    ) -> CommandResolutionContext<'a> {
        CommandResolutionContext {
            selected_units: units,
            target,
            world,
            unit_catalog,
            weapon_catalog,
            targeting_policy: AttackTargetingPolicy::default(),
        }
    }

    #[test]
    fn terrain_click_resolves_to_move() {
        let world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let unit_catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let units = [UnitId::new(1)];
        let resolved = resolve_contextual_command(&ctx(
            &units,
            CommandTarget::Terrain { position: pos(10.0, 10.0) },
            &world,
            &unit_catalog,
            &weapons,
        ))
        .unwrap();
        assert_eq!(resolved.command_type, CommandType::Move);
    }

    #[test]
    fn hostile_unit_click_resolves_to_attack() {
        let unit_catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let player = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let hostile = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(5.0, 5.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap()
        .id;
        let resolved = resolve_contextual_command(&ctx(
            &[player],
            CommandTarget::Unit { unit_id: hostile },
            &world,
            &unit_catalog,
            &weapons,
        ))
        .unwrap();
        assert_eq!(resolved.command_type, CommandType::Attack);
    }

    #[test]
    fn friendly_unit_click_resolves_to_move() {
        let unit_catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let a = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let b = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(5.0, 5.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let resolved = resolve_contextual_command(&ctx(
            &[a],
            CommandTarget::Unit { unit_id: b },
            &world,
            &unit_catalog,
            &weapons,
        ))
        .unwrap();
        assert_eq!(resolved.command_type, CommandType::Move);
    }

    #[test]
    fn empty_selection_returns_none() {
        let world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let unit_catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        assert!(resolve_contextual_command(&ctx(
            &[],
            CommandTarget::Terrain { position: pos(0.0, 0.0) },
            &world,
            &unit_catalog,
            &weapons,
        ))
        .is_none());
    }
}
