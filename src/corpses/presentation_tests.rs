//! Corpse presentation and handoff regressions.

use bevy::prelude::*;

use crate::corpses::{
    CorpseRenderEntity, CorpseRenderIndex, CorpseSceneRoot, handoff_unit_render_to_corpse,
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
