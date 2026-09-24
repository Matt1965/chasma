//! Unit interaction menu row rules.

use super::content::{build_interaction_menu_rows, should_omit_interaction_menu_option};
use crate::world::dialogue::config::{DialogueActionKind, DialogueOptionRule, UnitDialogueConfig};
use crate::world::relationship::AuthoredRelationshipCatalog;
use crate::world::{
    Affiliation, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DialogueUnavailableReason,
    Heightfield, LocalPosition, UnitDefinitionId, UnitId, UnitOwnership, UnitPlacement, UnitRecord,
    UnitSource, WorldData, WorldPosition,
};
use bevy::prelude::{Quat, Vec3};

fn flat_world() -> WorldData {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
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

fn npc_with_dialogue(talk: bool, trade: bool, recruit: bool) -> (WorldData, UnitId, UnitId) {
    let mut world = flat_world();
    let actor = UnitId::new(1);
    let target = UnitId::new(2);
    let actor_record = UnitRecord::new(
        actor,
        UnitDefinitionId::new("player_unit"),
        UnitPlacement::new(pos(0.0, 0.0), Quat::IDENTITY),
        UnitSource::Authored,
        UnitOwnership::player_default(),
        100,
        crate::world::FactionId::new("player"),
        crate::world::SpeciesId::new("human"),
    );
    let mut target_record = UnitRecord::new(
        target,
        UnitDefinitionId::new("npc"),
        UnitPlacement::new(pos(1.0, 0.0), Quat::IDENTITY),
        UnitSource::Authored,
        UnitOwnership::neutral(),
        100,
        crate::world::FactionId::new("neutral"),
        crate::world::SpeciesId::new("human"),
    );
    target_record.dialogue = Some(UnitDialogueConfig {
        talk: DialogueOptionRule {
            enabled: talk,
            min_relationship: 0,
        },
        trade: DialogueOptionRule {
            enabled: trade,
            min_relationship: 50,
        },
        recruit: DialogueOptionRule {
            enabled: recruit,
            min_relationship: 75,
        },
    });
    world
        .insert_unit(ChunkId::new(ChunkCoord::new(0, 0)), actor_record)
        .unwrap();
    world
        .insert_unit(ChunkId::new(ChunkCoord::new(0, 0)), target_record)
        .unwrap();
    (world, actor, target)
}

#[test]
fn omits_disabled_capabilities() {
    let (world, actor, target) = npc_with_dialogue(true, false, true);
    let rows = build_interaction_menu_rows(
        &world,
        &AuthoredRelationshipCatalog::default(),
        world.relationship_standing_store(),
        actor,
        target,
    );
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row.kind == DialogueActionKind::Talk && row.enabled));
    assert!(rows.iter().any(|row| row.kind == DialogueActionKind::Recruit && !row.enabled));
    assert!(!rows.iter().any(|row| row.kind == DialogueActionKind::Trade));
}

#[test]
fn shows_relationship_gate_as_disabled_reason() {
    let (world, actor, target) = npc_with_dialogue(true, true, true);
    let rows = build_interaction_menu_rows(
        &world,
        &AuthoredRelationshipCatalog::default(),
        world.relationship_standing_store(),
        actor,
        target,
    );
    let trade = rows
        .iter()
        .find(|row| row.kind == DialogueActionKind::Trade)
        .expect("trade row");
    assert!(!trade.enabled);
    assert!(trade.label.contains("Requires better relationship"));
}

#[test]
fn omit_helper_covers_capability_reasons() {
    assert!(should_omit_interaction_menu_option(DialogueUnavailableReason::TargetCannotTrade));
    assert!(!should_omit_interaction_menu_option(DialogueUnavailableReason::RelationshipTooLow));
}
