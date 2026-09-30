//! Interaction menu row model — omits unsupported capabilities, shows gated options disabled.

use crate::world::{
    is_player_controllable, is_unit_alive,
    AuthoredRelationshipCatalog, DialogueActionKind, DialogueOptionAvailability,
    DialogueUnavailableReason, RelationshipStandingStore, UnitCatalog, UnitId, WorldData,
    evaluate_dialogue_option,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionMenuRow {
    pub kind: DialogueActionKind,
    pub label: String,
    pub enabled: bool,
}

pub fn should_omit_interaction_menu_option(reason: DialogueUnavailableReason) -> bool {
    matches!(
        reason,
        DialogueUnavailableReason::DialogueNotSupported
            | DialogueUnavailableReason::OptionDisabled
            | DialogueUnavailableReason::TargetCannotTrade
            | DialogueUnavailableReason::TargetCannotBeRecruited
            | DialogueUnavailableReason::TargetIsPlayerUnit
    )
}

/// Whether a right-click between these units should open the contextual interaction menu.
pub fn unit_accepts_context_interaction_menu(
    world: &WorldData,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) -> bool {
    if actor_unit_id == target_unit_id {
        return false;
    }
    world
        .get_unit(target_unit_id)
        .is_some_and(|record| is_unit_alive(record))
}

/// Both units are player-controllable (e.g. two squad members).
pub fn is_player_to_player_trade_pair(
    world: &WorldData,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) -> bool {
    if actor_unit_id == target_unit_id {
        return false;
    }
    let actor = world.get_unit(actor_unit_id);
    let target = world.get_unit(target_unit_id);
    actor.is_some_and(is_player_controllable) && target.is_some_and(is_player_controllable)
}

/// Trade-only compact menu for player squads without authored social capabilities on the target.
pub fn uses_player_trade_only_menu(
    world: &WorldData,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) -> bool {
    if !is_player_to_player_trade_pair(world, actor_unit_id, target_unit_id) {
        return false;
    }
    world
        .get_unit(target_unit_id)
        .and_then(|record| record.dialogue.as_ref())
        .is_none_or(|config| !config.has_any_enabled())
}

pub fn interaction_menu_option_selectable(
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
    kind: DialogueActionKind,
) -> bool {
    if uses_player_trade_only_menu(world, actor_unit_id, target_unit_id) {
        return kind == DialogueActionKind::Trade;
    }
    evaluate_dialogue_option(
        world,
        authored_relationships,
        standing,
        actor_unit_id,
        target_unit_id,
        kind,
    )
    .is_available()
}

pub fn build_interaction_menu_rows(
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) -> Vec<InteractionMenuRow> {
    if !unit_accepts_context_interaction_menu(world, actor_unit_id, target_unit_id) {
        return Vec::new();
    }

    if uses_player_trade_only_menu(world, actor_unit_id, target_unit_id) {
        return vec![InteractionMenuRow {
            kind: DialogueActionKind::Trade,
            label: DialogueActionKind::Trade.label().to_string(),
            enabled: true,
        }];
    }

    let Some(target) = world.get_unit(target_unit_id) else {
        return Vec::new();
    };
    let Some(config) = target.dialogue.as_ref() else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    for kind in DialogueActionKind::ALL {
        if !config.rule(kind).enabled {
            continue;
        }
        let availability = evaluate_dialogue_option(
            world,
            authored_relationships,
            standing,
            actor_unit_id,
            target_unit_id,
            kind,
        );
        match availability {
            DialogueOptionAvailability::Available => rows.push(InteractionMenuRow {
                kind,
                label: kind.label().to_string(),
                enabled: true,
            }),
            DialogueOptionAvailability::Unavailable(reason) => {
                if should_omit_interaction_menu_option(reason) {
                    continue;
                }
                rows.push(InteractionMenuRow {
                    kind,
                    label: format!("{} — {}", kind.label(), reason.player_message(kind)),
                    enabled: false,
                });
            }
        }
    }
    rows
}

pub fn interaction_menu_title(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    target_unit_id: UnitId,
) -> String {
    crate::ui::gameplay::dialogue::target_display_name(world, unit_catalog, target_unit_id)
}
