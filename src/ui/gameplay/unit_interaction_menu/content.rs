//! Interaction menu row model — omits unsupported capabilities, shows gated options disabled.

use crate::ui::gameplay::dialogue::target_display_name;
use crate::world::{
    AuthoredRelationshipCatalog, DialogueActionKind, DialogueOptionAvailability,
    DialogueUnavailableReason, RelationshipStandingStore, UnitCatalog, UnitId, WorldData,
    present_dialogue_options,
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

pub fn build_interaction_menu_rows(
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) -> Vec<InteractionMenuRow> {
    present_dialogue_options(
        world,
        authored_relationships,
        standing,
        actor_unit_id,
        target_unit_id,
    )
    .into_iter()
    .filter_map(|(kind, availability)| match availability {
        DialogueOptionAvailability::Available => Some(InteractionMenuRow {
            kind,
            label: kind.label().to_string(),
            enabled: true,
        }),
        DialogueOptionAvailability::Unavailable(reason)
            if should_omit_interaction_menu_option(reason) =>
        {
            None
        }
        DialogueOptionAvailability::Unavailable(reason) => Some(InteractionMenuRow {
            kind,
            label: format!("{} — {}", kind.label(), reason.player_message(kind)),
            enabled: false,
        }),
    })
    .collect()
}

pub fn interaction_menu_title(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    target_unit_id: UnitId,
) -> String {
    target_display_name(world, unit_catalog, target_unit_id)
}
