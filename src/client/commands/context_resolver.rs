//! Context resolution — classifies clicks into [`ContextualCommandIntent`] (ADR-041 U-UI5, ADR-056 C3).

use bevy::prelude::Vec3;

use crate::world::{
    AttackTargetingPolicy, UnitCatalog, WeaponCatalog, WorldData,
    is_valid_autonomous_attack_target, is_valid_explicit_attack_target,
};

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
    pub authored_relationships: &'a crate::world::AuthoredRelationshipCatalog,
    pub targeting_policy: AttackTargetingPolicy,
}

/// Classify a command target given the current selection.
///
/// Returns `None` when the click cannot produce a command (empty selection).
pub fn resolve_contextual_command(
    ctx: &CommandResolutionContext<'_>,
) -> Option<ContextualCommandIntent> {
    resolve_contextual_command_with_armed(ctx, None)
}

/// Resolve a contextual command, honoring an armed palette command when set.
pub fn resolve_contextual_command_with_armed(
    ctx: &CommandResolutionContext<'_>,
    armed: Option<CommandType>,
) -> Option<ContextualCommandIntent> {
    if ctx.selected_units.is_empty() {
        return None;
    }

    if let Some(armed_type) = armed {
        return match armed_type {
            CommandType::Attack => match ctx.target {
                CommandTarget::Unit { unit_id }
                    if any_selected_can_explicit_attack(ctx, unit_id) =>
                {
                    Some(ContextualCommandIntent {
                        command_type: CommandType::Attack,
                        target: CommandTarget::Unit { unit_id },
                    })
                }
                CommandTarget::Terrain { position } => Some(ContextualCommandIntent {
                    command_type: CommandType::AttackMove,
                    target: CommandTarget::Terrain { position },
                }),
                _ => None,
            },
            CommandType::Move => Some(ContextualCommandIntent {
                command_type: CommandType::Move,
                target: ctx.target,
            }),
            CommandType::AttackMove => match ctx.target {
                CommandTarget::Terrain { position } => Some(ContextualCommandIntent {
                    command_type: CommandType::AttackMove,
                    target: CommandTarget::Terrain { position },
                }),
                _ => None,
            },
            _ => None,
        };
    }

    match &ctx.target {
        CommandTarget::Terrain { position } => Some(ContextualCommandIntent {
            command_type: CommandType::Move,
            target: CommandTarget::Terrain {
                position: *position,
            },
        }),
        CommandTarget::Unit { unit_id } => {
            let attacker = *ctx.selected_units.first()?;
            if is_valid_autonomous_attack_target(
                ctx.world,
                ctx.authored_relationships,
                attacker,
                *unit_id,
                ctx.weapon_catalog,
                ctx.unit_catalog,
                ctx.targeting_policy,
            ) {
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

fn any_selected_can_explicit_attack(ctx: &CommandResolutionContext<'_>, target: UnitId) -> bool {
    ctx.selected_units.iter().any(|attacker| {
        is_valid_explicit_attack_target(
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

    Some(ContextualCommandIntent {
        command_type,
        target,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::relationship::{
        AuthoredFacetKey, AuthoredRelationshipCatalog, DirectedRelationshipEdgeKey, FactionId,
    };
    use crate::world::{
        ChunkCoord, ChunkLayout, LocalPosition, UnitDefinitionId, UnitOwnership, UnitSource,
        WorldData, WorldPosition, create_unit_with_ownership,
    };
    use bevy::prelude::Vec3;

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn authored() -> AuthoredRelationshipCatalog {
        AuthoredRelationshipCatalog::default()
    }

    fn player_attack_authored() -> AuthoredRelationshipCatalog {
        AuthoredRelationshipCatalog::from_edges([(
            DirectedRelationshipEdgeKey::new(
                AuthoredFacetKey::Faction(FactionId::new("player")),
                AuthoredFacetKey::Faction(FactionId::new("wild")),
            ),
            -150,
        )])
        .expect("valid player attack edge")
    }

    fn patch_player_faction(world: &mut WorldData, unit_id: UnitId) {
        let mut record = world.remove_unit_by_id(unit_id).expect("unit exists");
        record.faction_id = FactionId::new("player");
        let chunk = crate::world::ChunkId::new(record.placement.position.chunk);
        world.insert_unit(chunk, record).unwrap();
    }

    fn ctx<'a>(
        units: &'a [UnitId],
        target: CommandTarget,
        world: &'a WorldData,
        unit_catalog: &'a UnitCatalog,
        weapon_catalog: &'a WeaponCatalog,
        authored: &'a crate::world::AuthoredRelationshipCatalog,
    ) -> CommandResolutionContext<'a> {
        CommandResolutionContext {
            selected_units: units,
            target,
            world,
            unit_catalog,
            weapon_catalog,
            authored_relationships: authored,
            targeting_policy: AttackTargetingPolicy::default(),
        }
    }

    #[test]
    fn neutral_unit_default_click_resolves_to_move() {
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
        let neutral = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("deer"),
            pos(5.0, 5.0),
            UnitSource::Authored,
            UnitOwnership::neutral(),
        )
        .unwrap()
        .id;
        let resolved = resolve_contextual_command(&ctx(
            &[player],
            CommandTarget::Unit { unit_id: neutral },
            &world,
            &unit_catalog,
            &weapons,
            &authored(),
        ))
        .unwrap();
        assert_eq!(resolved.command_type, CommandType::Move);
    }

    #[test]
    fn armed_attack_on_neutral_resolves_to_attack() {
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
        let neutral = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("deer"),
            pos(5.0, 5.0),
            UnitSource::Authored,
            UnitOwnership::neutral(),
        )
        .unwrap()
        .id;
        let resolved = resolve_contextual_command_with_armed(
            &ctx(
                &[player],
                CommandTarget::Unit { unit_id: neutral },
                &world,
                &unit_catalog,
                &weapons,
                &authored(),
            ),
            Some(CommandType::Attack),
        )
        .unwrap();
        assert_eq!(resolved.command_type, CommandType::Attack);
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
            CommandTarget::Terrain {
                position: pos(10.0, 10.0),
            },
            &world,
            &unit_catalog,
            &weapons,
            &authored(),
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
        patch_player_faction(&mut world, player);
        let hostile = create_unit_with_ownership(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(5.0, 5.0),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
        )
        .unwrap()
        .id;
        let resolved = resolve_contextual_command(&ctx(
            &[player],
            CommandTarget::Unit { unit_id: hostile },
            &world,
            &unit_catalog,
            &weapons,
            &player_attack_authored(),
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
            &authored(),
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
        assert!(
            resolve_contextual_command(&ctx(
                &[],
                CommandTarget::Terrain {
                    position: pos(0.0, 0.0)
                },
                &world,
                &unit_catalog,
                &weapons,
                &authored(),
            ))
            .is_none()
        );
    }

    #[test]
    fn armed_attack_on_terrain_resolves_to_attack_move() {
        let world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let unit_catalog = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let units = [UnitId::new(1)];
        let resolved = resolve_contextual_command_with_armed(
            &ctx(
                &units,
                CommandTarget::Terrain {
                    position: pos(12.0, 8.0),
                },
                &world,
                &unit_catalog,
                &weapons,
                &authored(),
            ),
            Some(CommandType::Attack),
        )
        .unwrap();
        assert_eq!(resolved.command_type, CommandType::AttackMove);
    }
}
