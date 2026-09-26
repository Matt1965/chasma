//! Dialogue panel presentation model.

use crate::world::{
    AuthoredRelationshipCatalog, DialogueActionKind, DialogueOptionAvailability,
    RelationshipStandingStore, UnitCatalog, UnitId, WorldData, present_dialogue_options,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueOptionRow {
    pub kind: DialogueActionKind,
    pub label: String,
    pub enabled: bool,
    pub reason: Option<String>,
}

pub fn build_dialogue_option_rows(
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    standing: &RelationshipStandingStore,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) -> Vec<DialogueOptionRow> {
    present_dialogue_options(
        world,
        authored_relationships,
        standing,
        actor_unit_id,
        target_unit_id,
    )
    .into_iter()
    .map(|(kind, availability)| {
        let (enabled, reason) = match availability {
            DialogueOptionAvailability::Available => (true, None),
            DialogueOptionAvailability::Unavailable(reason) => (
                false,
                Some(reason.player_message(kind).to_string()),
            ),
        };
        DialogueOptionRow {
            kind,
            label: kind.label().to_string(),
            enabled,
            reason,
        }
    })
    .collect()
}

pub fn target_display_name(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    target_unit_id: UnitId,
) -> String {
    world
        .get_unit(target_unit_id)
        .and_then(|record| unit_catalog.get(&record.definition_id))
        .map(|definition| definition.display_name.clone())
        .unwrap_or_else(|| "Unknown".to_string())
}
