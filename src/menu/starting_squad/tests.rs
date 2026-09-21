use crate::world::{OriginId, seed_origin_catalog};

use super::draft::StartingSquadDraft;
use super::session::StartingSquadSession;

fn dev_catalogs() -> (
    crate::world::UnitCatalog,
    crate::world::AppearanceProfileCatalog,
) {
    let appearance_profiles = crate::data_import::resolve_dev_appearance_profile_catalog();
    let factions = crate::data_import::resolve_dev_faction_catalog();
    let species = crate::data_import::resolve_dev_species_catalog();
    let weapons = crate::data_import::resolve_dev_weapon_catalog();
    let animation_profiles = crate::data_import::resolve_dev_animation_profile_catalog();
    let inventory_profiles = crate::data_import::resolve_dev_inventory_profile_catalog();
    let units = crate::data_import::resolve_dev_unit_catalog(
        &factions,
        &species,
        &weapons,
        &animation_profiles,
        &inventory_profiles,
        &appearance_profiles,
        None,
    );
    (units, appearance_profiles)
}

#[test]
fn new_game_session_selects_first_origin_and_builds_draft() {
    let (units, profiles) = dev_catalogs();
    let origins = seed_origin_catalog(&units, &profiles);
    let session =
        StartingSquadSession::new_for_first_origin(&origins, &units, &profiles).unwrap();
    assert_eq!(session.selected_origin_index, 0);
    let draft = session.active_draft(&origins).unwrap();
    assert_eq!(draft.origin_id, OriginId::new("exile_pair"));
    assert_eq!(draft.members.len(), 2);
}

#[test]
fn origin_cycle_wraps_and_retains_per_origin_drafts() {
    let (units, profiles) = dev_catalogs();
    let origins = seed_origin_catalog(&units, &profiles);
    let mut session =
        StartingSquadSession::new_for_first_origin(&origins, &units, &profiles).unwrap();
    let first = session.active_draft(&origins).unwrap().members[0]
        .appearance
        .appearance
        .height_scale;
    session
        .active_draft_mut(&origins)
        .unwrap()
        .members[0]
        .appearance
        .appearance
        .height_scale = first + 0.1;
    session
        .active_draft_mut(&origins)
        .unwrap()
        .members[0]
        .edited = true;

    session.cycle_origin(1, &origins, &units, &profiles).unwrap();
    assert_eq!(
        session
            .active_origin_id(&origins)
            .map(|id| id.as_str().to_string()),
        Some("lone_survivor".to_string())
    );
    session.cycle_origin(-1, &origins, &units, &profiles).unwrap();
    let restored = session.active_draft(&origins).unwrap();
    assert_eq!(restored.origin_id, OriginId::new("exile_pair"));
    assert!(restored.members[0].edited);
    assert!(
        (restored.members[0].appearance.appearance.height_scale - (first + 0.1)).abs()
            < 1e-5
    );
}

#[test]
fn draft_members_have_independent_appearance_state() {
    let (units, profiles) = dev_catalogs();
    let origins = seed_origin_catalog(&units, &profiles);
    let origin = origins.get_index(0).unwrap();
    let mut draft =
        StartingSquadDraft::from_origin_definition(origin, &units, &profiles).unwrap();
    draft.members[0].appearance.appearance.height_scale = 1.2;
    assert_ne!(
        draft.members[0].appearance.appearance.height_scale,
        draft.members[1].appearance.appearance.height_scale
    );
}

#[test]
fn begin_game_spawn_creates_configured_units() {
    let (units, profiles) = dev_catalogs();
    let origins = seed_origin_catalog(&units, &profiles);
    let origin = origins.get_index(0).unwrap();
    let draft = StartingSquadDraft::from_origin_definition(origin, &units, &profiles).unwrap();
    let mut world = crate::world::WorldData::new(crate::world::ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield =
        crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        crate::world::ChunkId::new(crate::world::ChunkCoord::new(0, 0)),
        crate::world::ChunkData::new(heightfield, vec![]),
    );
    let (item_categories, item_catalog) = crate::data_import::resolve_dev_item_catalog();
    let inventory_profiles = crate::data_import::resolve_dev_inventory_profile_catalog();
    let inventory_ctx =
        crate::world::InventoryCatalogCtx::new(&item_catalog, &item_categories, &inventory_profiles);
    let anchor = origin.spawn_anchor();
    let spawned = super::spawn::spawn_starting_squad_from_draft(
        &mut world,
        &units,
        &profiles,
        &inventory_ctx,
        &anchor,
        &draft,
    )
    .unwrap();
    assert_eq!(spawned.len(), draft.members.len());
    for (unit_id, member) in spawned.iter().zip(draft.members.iter()) {
        let record = world.get_unit(*unit_id).unwrap();
        assert_eq!(record.definition_id, member.definition_id);
        assert_eq!(
            record.appearance.as_ref().map(|value| value.height_scale),
            Some(member.appearance.appearance.height_scale)
        );
    }
}

#[test]
fn origin_change_clears_focus_mode() {
    let (units, profiles) = dev_catalogs();
    let origins = seed_origin_catalog(&units, &profiles);
    let mut session =
        StartingSquadSession::new_for_first_origin(&origins, &units, &profiles).unwrap();
    session.enter_focus(0).unwrap();
    session
        .cycle_origin(1, &origins, &units, &profiles)
        .unwrap();
    assert!(!session.is_focused());
}
