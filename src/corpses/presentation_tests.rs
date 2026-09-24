//! Corpse presentation and handoff regressions.

use bevy::prelude::*;

use crate::corpses::{
    CorpsePresentationClaim, CorpseRenderEntity, CorpseRenderIndex, CorpseSceneRoot,
    claim_corpse_presentation_for_death, corpse_presentation_entity_count,
    handoff_unit_render_to_corpse, should_spawn_corpse_presentation,
};
use crate::units::DeathPresentation;
use crate::units::{UnitRenderEntity, UnitRenderIndex, UnitSceneRoot};
use crate::world::{
    Affiliation, ChunkCoord, ChunkData, ChunkId, ChunkLayout, CorpseId, CorpseRecord,
    CorpseSettings, CorpseState, Heightfield, LocalPosition, UnitDefinitionId, UnitId,
    UnitPlacement, WorldData, WorldPosition, create_corpse_from_unit, nearest_corpse_at_position,
};
use crate::world::{UnitCatalog, UnitOwnership, UnitSource, starter_unit_definitions};

fn flat_world() -> WorldData {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
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

fn test_corpse_origin_has_pending_death_root(
    bevy_world: &mut World,
    world_data: &WorldData,
    origin_unit_id: UnitId,
) -> bool {
    if world_data.get_unit(origin_unit_id).is_some() {
        return false;
    }
    let mut unit_render_roots = bevy_world.query::<&UnitRenderEntity>();
    for marker in unit_render_roots.iter(bevy_world) {
        if marker.unit_id == origin_unit_id {
            return true;
        }
    }
    let mut death_presentations = bevy_world.query::<&DeathPresentation>();
    for presentation in death_presentations.iter(bevy_world) {
        if presentation.origin_unit_id == Some(origin_unit_id) {
            return true;
        }
    }
    false
}

fn test_release_stale_corpse_presentation_owners(
    bevy_world: &mut World,
    corpse_index: &mut CorpseRenderIndex,
) -> Vec<CorpseId> {
    let mut entities = bevy_world.query::<Entity>();
    let mut claims = bevy_world.query::<&CorpsePresentationClaim>();
    let mut death_presentations = bevy_world.query::<&DeathPresentation>();
    let mut corpse_render_entities = bevy_world.query::<&CorpseRenderEntity>();
    let stale_ids: Vec<CorpseId> = corpse_index
        .0
        .iter()
        .filter_map(|(corpse_id, entity)| {
            if entities.get(bevy_world, *entity).is_err() {
                return Some(*corpse_id);
            }
            if claims.get(bevy_world, *entity).is_ok()
                && death_presentations.get(bevy_world, *entity).is_err()
                && corpse_render_entities.get(bevy_world, *entity).is_err()
            {
                return Some(*corpse_id);
            }
            None
        })
        .collect();
    for corpse_id in &stale_ids {
        corpse_index.0.remove(corpse_id);
    }
    stale_ids
}

#[test]
fn create_corpse_copies_unit_appearance() {
    use crate::world::{
        AppearanceParamId, AppearanceProfileId, BodyVariantId, UnitAppearance,
        create_unit_with_ownership,
    };

    let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let mut world = flat_world();
    let appearance = UnitAppearance {
        profile_id: AppearanceProfileId::new("humanoid"),
        body_variant_id: BodyVariantId::new("male"),
        height_scale: 1.05,
        morphs: [(AppearanceParamId::new("build"), 0.5)].into(),
        generation_seed: Some(9),
    };
    let unit = create_unit_with_ownership(
        &catalog,
        &crate::world::AppearanceProfileCatalog::empty(),
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(1.0, 1.0),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
    )
    .unwrap();
    world.mutate_unit(unit.id, |record| {
        record.appearance = Some(appearance.clone());
    });

    let definition = catalog.get(&unit.definition_id).unwrap();
    let unit_record = world.get_unit(unit.id).unwrap().clone();
    let corpse = create_corpse_from_unit(
        &mut world,
        &unit_record,
        definition,
        &CorpseSettings::default(),
        1,
    )
    .unwrap();
    assert_eq!(corpse.appearance, Some(appearance));
}

#[test]
fn death_handoff_retargets_render_entity() {
    let mut world = World::new();
    world.init_resource::<UnitRenderIndex>();
    world.init_resource::<CorpseRenderIndex>();

    let unit_id = UnitId::new(1);
    let corpse_id = CorpseId::new(2);
    let entity = world
        .spawn((
            UnitRenderEntity { unit_id },
            UnitSceneRoot,
            DeathPresentation {
                origin_unit_id: Some(unit_id),
                definition_id: UnitDefinitionId::new("bandit"),
                profile_id: crate::world::AnimationProfileId::new("humanoid"),
                remaining_seconds: 0.0,
                freeze_pose: false,
            },
        ))
        .id();
    world
        .resource_mut::<UnitRenderIndex>()
        .0
        .insert(unit_id, entity);

    let mut unit_index = world.remove_resource::<UnitRenderIndex>().unwrap();
    let mut corpse_index = world.remove_resource::<CorpseRenderIndex>().unwrap();
    handoff_unit_render_to_corpse(
        &mut world.commands(),
        &mut unit_index,
        &mut corpse_index,
        entity,
        unit_id,
        corpse_id,
    );
    world.insert_resource(unit_index);
    world.insert_resource(corpse_index);
    world.flush();

    assert!(world.get::<UnitRenderEntity>(entity).is_none());
    assert!(world.get::<CorpseRenderEntity>(entity).is_some());
    assert!(world.get::<CorpseSceneRoot>(entity).is_some());
    assert!(world.get::<DeathPresentation>(entity).is_none());
    assert_eq!(
        world.resource::<CorpseRenderIndex>().0.get(&corpse_id),
        Some(&entity)
    );
}

#[test]
fn claimed_death_presentation_blocks_corpse_reconstruction_spawn() {
    let mut world_data = flat_world();
    let unit_id = UnitId::new(42);
    let corpse_id = CorpseId::new(7);
    let click = pos(10.0, 10.0);
    let record = CorpseRecord::new(
        corpse_id,
        unit_id,
        UnitDefinitionId::new("bandit"),
        UnitPlacement::new(click, Quat::IDENTITY),
        crate::world::SpaceId::SURFACE,
        None,
        None,
        None,
        None,
        None,
        Affiliation::Unknown,
        0,
        100,
    );
    world_data
        .corpse_store_mut()
        .insert(ChunkId::new(click.chunk), record)
        .unwrap();

    let mut bevy_world = World::new();
    bevy_world.init_resource::<CorpseRenderIndex>();
    let entity = bevy_world
        .spawn((
            UnitRenderEntity { unit_id },
            UnitSceneRoot,
            DeathPresentation {
                origin_unit_id: Some(unit_id),
                definition_id: UnitDefinitionId::new("bandit"),
                profile_id: crate::world::AnimationProfileId::new("humanoid"),
                remaining_seconds: 1.5,
                freeze_pose: false,
            },
        ))
        .id();

    bevy_world.resource_scope(|world, mut corpse_index: Mut<CorpseRenderIndex>| {
        claim_corpse_presentation_for_death(
            &mut corpse_index,
            &mut world.commands(),
            entity,
            corpse_id,
            unit_id,
        );
    });
    bevy_world.flush();

    let origin_pending =
        test_corpse_origin_has_pending_death_root(&mut bevy_world, &world_data, unit_id);
    assert!(origin_pending);

    let corpse_index = bevy_world.resource::<CorpseRenderIndex>();
    assert_eq!(corpse_presentation_entity_count(corpse_index, corpse_id), 1);
    assert!(bevy_world.get::<CorpsePresentationClaim>(entity).is_some());
    assert!(!should_spawn_corpse_presentation(
        &world_data,
        corpse_id,
        corpse_index,
        origin_pending,
    ));
}

#[test]
fn death_handoff_keeps_single_presentation_entity() {
    let mut world = World::new();
    world.init_resource::<UnitRenderIndex>();
    world.init_resource::<CorpseRenderIndex>();

    let unit_id = UnitId::new(1);
    let corpse_id = CorpseId::new(2);
    let entity = world
        .spawn((
            UnitRenderEntity { unit_id },
            UnitSceneRoot,
            CorpsePresentationClaim {
                corpse_id,
                origin_unit_id: unit_id,
            },
            DeathPresentation {
                origin_unit_id: Some(unit_id),
                definition_id: UnitDefinitionId::new("bandit"),
                profile_id: crate::world::AnimationProfileId::new("humanoid"),
                remaining_seconds: 0.0,
                freeze_pose: false,
            },
        ))
        .id();
    world
        .resource_mut::<CorpseRenderIndex>()
        .0
        .insert(corpse_id, entity);

    let mut unit_index = world.remove_resource::<UnitRenderIndex>().unwrap();
    let mut corpse_index = world.remove_resource::<CorpseRenderIndex>().unwrap();
    handoff_unit_render_to_corpse(
        &mut world.commands(),
        &mut unit_index,
        &mut corpse_index,
        entity,
        unit_id,
        corpse_id,
    );
    world.insert_resource(unit_index);
    world.insert_resource(corpse_index);
    world.flush();

    let corpse_index = world.resource::<CorpseRenderIndex>();
    assert_eq!(corpse_index.0.get(&corpse_id), Some(&entity));
    assert_eq!(corpse_presentation_entity_count(corpse_index, corpse_id), 1);
    assert!(world.get::<CorpseRenderEntity>(entity).is_some());
    assert!(world.get::<CorpsePresentationClaim>(entity).is_none());
}

#[test]
fn stale_claim_release_allows_corpse_reconstruction() {
    let mut world_data = flat_world();
    let unit_id = UnitId::new(55);
    let corpse_id = CorpseId::new(9);
    let click = pos(12.0, 12.0);
    let record = CorpseRecord::new(
        corpse_id,
        unit_id,
        UnitDefinitionId::new("bandit"),
        UnitPlacement::new(click, Quat::IDENTITY),
        crate::world::SpaceId::SURFACE,
        None,
        None,
        None,
        None,
        None,
        Affiliation::Unknown,
        0,
        100,
    );
    world_data
        .corpse_store_mut()
        .insert(ChunkId::new(click.chunk), record)
        .unwrap();

    let mut bevy_world = World::new();
    bevy_world.init_resource::<CorpseRenderIndex>();
    let missing_entity = Entity::from_bits(999_999);
    bevy_world
        .resource_mut::<CorpseRenderIndex>()
        .0
        .insert(corpse_id, missing_entity);

    let released = bevy_world.resource_scope(|world, mut corpse_index: Mut<CorpseRenderIndex>| {
        test_release_stale_corpse_presentation_owners(world, &mut corpse_index)
    });
    assert_eq!(released, vec![corpse_id]);

    let origin_pending =
        test_corpse_origin_has_pending_death_root(&mut bevy_world, &world_data, unit_id);

    let corpse_index = bevy_world.resource::<CorpseRenderIndex>();
    assert_eq!(corpse_presentation_entity_count(corpse_index, corpse_id), 0);
    assert!(should_spawn_corpse_presentation(
        &world_data,
        corpse_id,
        corpse_index,
        origin_pending,
    ));
}

#[test]
fn nearest_corpse_query_ignores_expired() {
    let mut world = flat_world();
    let corpse_id = CorpseId::new(1);
    let click = pos(5.0, 5.0);
    let record = CorpseRecord::new(
        corpse_id,
        UnitId::new(9),
        UnitDefinitionId::new("bandit"),
        UnitPlacement::new(click, Quat::IDENTITY),
        crate::world::SpaceId::SURFACE,
        None,
        None,
        None,
        None,
        None,
        Affiliation::Unknown,
        0,
        100,
    );
    world
        .corpse_store_mut()
        .insert(ChunkId::new(click.chunk), record)
        .unwrap();
    world
        .corpse_store_mut()
        .get_mut(corpse_id)
        .unwrap()
        .state = CorpseState::Expired;
    assert!(
        nearest_corpse_at_position(
            &world,
            click,
            crate::world::SpaceId::SURFACE,
            &CorpseSettings::default(),
        )
        .is_none()
    );
}
