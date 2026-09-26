use super::*;
use crate::world::ownership::UnitOwnership;
use crate::world::relationship::{
    AuthoredFacetKey, AuthoredRelationshipCatalog, DirectedRelationshipEdgeKey, FactionId,
    RelationshipStandingStore, SpeciesId,
};
use crate::world::{
    Affiliation, ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition,
    UnitDefinitionId, UnitPlacement, UnitRecord, UnitSource, WorldData, WorldPosition,
    apply_unit_archetype_dialogue_config, resolve_unit_spawn_spec, UnitArchetypeCatalog,
    UnitArchetypeDefinition, UnitArchetypeId,
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

fn merchant_config() -> UnitDialogueConfig {
    UnitDialogueConfig {
        talk: DialogueOptionRule {
            enabled: true,
            min_relationship: -50,
        },
        trade: DialogueOptionRule {
            enabled: true,
            min_relationship: 25,
        },
        recruit: DialogueOptionRule {
            enabled: true,
            min_relationship: 100,
        },
    }
}

fn unit_record(id: u64, dialogue: Option<UnitDialogueConfig>) -> UnitRecord {
    let mut record = UnitRecord::new(
        crate::world::UnitId::new(id),
        UnitDefinitionId::new("bandit"),
        UnitPlacement::new(pos(0.0, 0.0), Quat::IDENTITY),
        UnitSource::Authored,
        UnitOwnership::neutral(),
        100,
        crate::world::FactionId::new("neutral"),
        SpeciesId::new("human"),
    );
    record.dialogue = dialogue;
    record
}

fn insert_units(world: &mut WorldData, actor: UnitRecord, target: UnitRecord) {
    world
        .insert_unit(ChunkId::new(ChunkCoord::new(0, 0)), actor)
        .unwrap();
    world
        .insert_unit(ChunkId::new(ChunkCoord::new(0, 0)), target)
        .unwrap();
}

fn authored_with_edge(value: i32) -> AuthoredRelationshipCatalog {
    AuthoredRelationshipCatalog::from_edges([(
        DirectedRelationshipEdgeKey::new(
            AuthoredFacetKey::Faction(FactionId::new("npc_faction")),
            AuthoredFacetKey::Faction(FactionId::new("player_faction")),
        ),
        value,
    )])
    .unwrap()
}

#[test]
fn old_archetype_deserializes_without_dialogue_field() {
    let ron = r#"(
        id: "legacy",
        display_name: "Legacy",
        applicable_species: ["human"],
        gold_min: 0,
        gold_max: 0,
        affiliation_override: None,
        equipment: [],
        inventory_stacks: [],
        enabled: true,
    )"#;
    let definition: UnitArchetypeDefinition = ron::from_str(ron).unwrap();
    assert!(definition.dialogue.is_none());
}

#[test]
fn archetype_dialogue_round_trips_through_ron() {
    let definition = UnitArchetypeDefinition {
        id: UnitArchetypeId::new("test_merchant"),
        display_name: "Test Merchant".to_string(),
        applicable_species: vec![SpeciesId::new("human")],
        gold_min: 0,
        gold_max: 0,
        affiliation_override: None,
        equipment: Vec::new(),
        inventory_stacks: Vec::new(),
        dialogue: Some(merchant_config()),
        enabled: true,
    };
    let serialized = ron::ser::to_string_pretty(&definition, Default::default()).unwrap();
    let restored: UnitArchetypeDefinition = ron::from_str(&serialized).unwrap();
    assert_eq!(restored.dialogue, Some(merchant_config()));
}

#[test]
fn archetype_spawn_bakes_dialogue_onto_unit_record() {
    let archetypes = UnitArchetypeCatalog::from_definitions(vec![UnitArchetypeDefinition {
        id: UnitArchetypeId::new("test_merchant"),
        display_name: "Test Merchant".to_string(),
        applicable_species: vec![SpeciesId::new("human")],
        gold_min: 0,
        gold_max: 0,
        affiliation_override: None,
        equipment: Vec::new(),
        inventory_stacks: Vec::new(),
        dialogue: Some(merchant_config()),
        enabled: true,
    }])
    .unwrap();
    let unit_catalog = crate::world::UnitCatalog::default();
    let spec = resolve_unit_spawn_spec(
        &UnitDefinitionId::new("bandit"),
        Some(&UnitArchetypeId::new("test_merchant")),
        Affiliation::Neutral,
        &unit_catalog,
        &archetypes,
    )
    .unwrap();
    assert_eq!(spec.dialogue, Some(merchant_config()));

    let mut world = flat_world();
    let record = unit_record(1, None);
    world
        .insert_unit(ChunkId::new(ChunkCoord::new(0, 0)), record)
        .unwrap();
    apply_unit_archetype_dialogue_config(&mut world, crate::world::UnitId::new(1), &spec);
    assert_eq!(
        world.get_unit(crate::world::UnitId::new(1)).unwrap().dialogue,
        Some(merchant_config())
    );
}

#[test]
fn changing_archetype_after_spawn_does_not_mutate_existing_unit() {
    let mut archetypes = UnitArchetypeCatalog::from_definitions(vec![UnitArchetypeDefinition {
        id: UnitArchetypeId::new("test_merchant"),
        display_name: "Test Merchant".to_string(),
        applicable_species: vec![SpeciesId::new("human")],
        gold_min: 0,
        gold_max: 0,
        affiliation_override: None,
        equipment: Vec::new(),
        inventory_stacks: Vec::new(),
        dialogue: Some(merchant_config()),
        enabled: true,
    }])
    .unwrap();
    let unit_catalog = crate::world::UnitCatalog::default();
    let spec = resolve_unit_spawn_spec(
        &UnitDefinitionId::new("bandit"),
        Some(&UnitArchetypeId::new("test_merchant")),
        Affiliation::Neutral,
        &unit_catalog,
        &archetypes,
    )
    .unwrap();
    let mut world = flat_world();
    let record = unit_record(1, None);
    world
        .insert_unit(ChunkId::new(ChunkCoord::new(0, 0)), record)
        .unwrap();
    apply_unit_archetype_dialogue_config(&mut world, crate::world::UnitId::new(1), &spec);

    let mut updated = archetypes.get(&UnitArchetypeId::new("test_merchant")).unwrap().clone();
    updated.dialogue = None;
    archetypes.upsert(updated).unwrap();
    assert!(world.get_unit(crate::world::UnitId::new(1)).unwrap().dialogue.is_some());
}

#[test]
fn relationship_gate_uses_npc_to_actor_direction() {
    let mut world = flat_world();
    let mut actor = unit_record(1, None);
    actor.faction_id = crate::world::FactionId::new("player_faction");
    let mut target = unit_record(2, Some(merchant_config()));
    target.faction_id = crate::world::FactionId::new("npc_faction");
    insert_units(&mut world, actor, target);

    let authored = authored_with_edge(30);
    let standing = RelationshipStandingStore::default();
    let availability = evaluate_dialogue_option(
        &world,
        &authored,
        &standing,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
        DialogueActionKind::Trade,
    );
    assert!(availability.is_available());

    let blocked = evaluate_dialogue_option(
        &world,
        &authored_with_edge(-100),
        &standing,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
        DialogueActionKind::Trade,
    );
    assert!(!blocked.is_available());

    let below = evaluate_dialogue_option(
        &world,
        &authored_with_edge(10),
        &standing,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
        DialogueActionKind::Trade,
    );
    assert!(!below.is_available());
}

#[test]
fn equal_threshold_is_available() {
    let mut world = flat_world();
    let actor = unit_record(1, None);
    let target = unit_record(2, Some(merchant_config()));
    insert_units(&mut world, actor, target);
    let authored = AuthoredRelationshipCatalog::default();
    let standing = RelationshipStandingStore::default();
    let availability = evaluate_dialogue_option(
        &world,
        &authored,
        &standing,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
        DialogueActionKind::Talk,
    );
    assert!(availability.is_available());
}

#[test]
fn disabled_options_are_independent() {
    let config = UnitDialogueConfig {
        talk: DialogueOptionRule {
            enabled: false,
            min_relationship: -100,
        },
        trade: DialogueOptionRule {
            enabled: true,
            min_relationship: 0,
        },
        recruit: DialogueOptionRule {
            enabled: false,
            min_relationship: 0,
        },
    };
    let mut world = flat_world();
    insert_units(
        &mut world,
        unit_record(1, None),
        unit_record(2, Some(config)),
    );
    let authored = AuthoredRelationshipCatalog::default();
    let standing = RelationshipStandingStore::default();
    assert!(!evaluate_dialogue_option(
        &world,
        &authored,
        &standing,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
        DialogueActionKind::Talk,
    )
    .is_available());
    assert!(evaluate_dialogue_option(
        &world,
        &authored,
        &standing,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
        DialogueActionKind::Trade,
    )
    .is_available());
}

#[test]
fn inventory_alone_does_not_enable_trade() {
    let mut world = flat_world();
    let mut actor = unit_record(1, None);
    actor.inventory_id = Some(crate::world::InventoryId::new(1));
    let target = unit_record(2, None);
    insert_units(&mut world, actor, target);
    let authored = AuthoredRelationshipCatalog::default();
    let standing = RelationshipStandingStore::default();
    assert!(!evaluate_dialogue_option(
        &world,
        &authored,
        &standing,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
        DialogueActionKind::Trade,
    )
    .is_available());
}

#[test]
fn player_controlled_target_blocks_recruit() {
    let mut world = flat_world();
    let actor = unit_record(1, None);
    let mut target = unit_record(2, Some(merchant_config()));
    target.affiliation = Affiliation::Player;
    target.owner_id = Some(crate::world::ownership::DEFAULT_PLAYER_OWNER_ID);
    insert_units(&mut world, actor, target);
    let authored = AuthoredRelationshipCatalog::default();
    let standing = RelationshipStandingStore::default();
    assert!(!evaluate_dialogue_option(
        &world,
        &authored,
        &standing,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
        DialogueActionKind::Recruit,
    )
    .is_available());
}

#[test]
fn different_space_blocks_interaction() {
    let mut world = flat_world();
    let actor = unit_record(1, None);
    let mut target = unit_record(2, Some(merchant_config()));
    target.current_space_id = crate::world::SpaceId::new(1);
    insert_units(&mut world, actor, target);
    let authored = AuthoredRelationshipCatalog::default();
    let standing = RelationshipStandingStore::default();
    assert!(!evaluate_dialogue_option(
        &world,
        &authored,
        &standing,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
        DialogueActionKind::Talk,
    )
    .is_available());
}
