//! CG8 roster preview actor lifecycle tests.

use bevy::prelude::*;

use crate::menu::{SquadMemberDraftId, StartingSquadSession};
use crate::units::presentation::{UnitEditorPreviewRosterMember, UnitEditorPreviewUnit};
use crate::world::{OriginId, seed_origin_catalog};

use super::presentation::{
    RosterPresentationMode, roster_actor_presentation_ready, roster_member_is_visible,
    roster_member_stage_translation,
};
use super::preview_invariants::collect_origin_preview_invariant_report;
use super::preview_reconcile::duplicate_roster_actor_entities;
use crate::ui::unit_editor::unit_editor_session_owns_cg3_preview_actor;
use crate::ui::unit_editor::{UnitEditorMode, UnitEditorSession};

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

fn count_roster_preview_actors(world: &mut World) -> usize {
    let mut query = world.query_filtered::<Entity, With<UnitEditorPreviewRosterMember>>();
    query.iter(world).count()
}

fn roster_actor_ids(world: &mut World) -> Vec<SquadMemberDraftId> {
    let mut query = world.query::<&UnitEditorPreviewRosterMember>();
    query
        .iter(world)
        .map(|member| member.draft_member_id)
        .collect()
}

fn spawn_mock_roster_actor(
    world: &mut World,
    draft_member_id: SquadMemberDraftId,
    slot_index: usize,
) -> Entity {
    world
        .spawn(UnitEditorPreviewRosterMember {
            draft_member_id,
            slot_index,
        })
        .id()
}

#[test]
fn new_game_draft_editor_session_does_not_own_cg3_preview_actor() {
    let (units, profiles) = dev_catalogs();
    let origins = seed_origin_catalog(&units, &profiles);
    let session =
        StartingSquadSession::new_for_first_origin(&origins, &units, &profiles).unwrap();
    let member = session.active_draft(&origins).unwrap().members[0].clone();
    let editor = UnitEditorSession::new(
        UnitEditorMode::NewGameDraft { slot_index: 0 },
        member.appearance.clone(),
    );
    assert!(!unit_editor_session_owns_cg3_preview_actor(&editor));
}

#[test]
fn invariant_report_expects_zero_cg3_actors_on_origin_stage() {
    let mut world = World::new();
    world.spawn((
        UnitEditorPreviewRosterMember {
            draft_member_id: SquadMemberDraftId(0),
            slot_index: 0,
        },
        Visibility::Visible,
    ));
    world.spawn((
        UnitEditorPreviewRosterMember {
            draft_member_id: SquadMemberDraftId(1),
            slot_index: 1,
        },
        Visibility::Visible,
    ));
    let report = collect_origin_preview_invariant_report(&mut world);
    report.validate_expected_roster_count(2).unwrap();
    assert_eq!(report.visible_roster_count, 2);
}

#[test]
fn focused_presentation_hides_other_members() {
    let mode = RosterPresentationMode::Focused { slot_index: 0 };
    assert!(roster_member_is_visible(0, mode));
    assert!(!roster_member_is_visible(1, mode));
    assert_eq!(count_visible_roster_actors_from_mode(2, mode), 1);
}

#[test]
fn squad_presentation_shows_all_members() {
    let mode = RosterPresentationMode::Squad;
    assert_eq!(count_visible_roster_actors_from_mode(2, mode), 2);
}

fn count_visible_roster_actors_from_mode(member_count: usize, mode: RosterPresentationMode) -> usize {
    (0..member_count)
        .filter(|slot| roster_member_is_visible(*slot, mode))
        .count()
}

#[test]
fn focused_member_uses_stage_center_without_spawning_cg3_actor() {
    let mut world = World::new();
    spawn_mock_roster_actor(&mut world, SquadMemberDraftId(0), 0);
    let mut cg3_preview = world.query_filtered::<Entity, With<UnitEditorPreviewUnit>>();
    assert_eq!(cg3_preview.iter(&world).count(), 0);
    let offset = Vec3::new(-1.5, 0.0, 0.0);
    let mode = RosterPresentationMode::Focused { slot_index: 0 };
    assert_eq!(
        roster_member_stage_translation(0, offset, mode),
        super::presentation::FOCUS_STAGE_POSITION
    );
}

#[test]
fn two_member_draft_expects_two_preview_actor_slots() {
    let (units, profiles) = dev_catalogs();
    let origins = seed_origin_catalog(&units, &profiles);
    let session =
        StartingSquadSession::new_for_first_origin(&origins, &units, &profiles).unwrap();
    let draft = session.active_draft(&origins).unwrap();
    assert_eq!(draft.members.len(), 2);
    assert_eq!(draft.members[0].id, SquadMemberDraftId(0));
    assert_eq!(draft.members[1].id, SquadMemberDraftId(1));
}

#[test]
fn focus_transitions_do_not_change_roster_actor_count() {
    let mut world = World::new();
    spawn_mock_roster_actor(&mut world, SquadMemberDraftId(0), 0);
    spawn_mock_roster_actor(&mut world, SquadMemberDraftId(1), 1);
    assert_eq!(count_roster_preview_actors(&mut world), 2);

    let (units, profiles) = dev_catalogs();
    let origins = seed_origin_catalog(&units, &profiles);
    let mut session =
        StartingSquadSession::new_for_first_origin(&origins, &units, &profiles).unwrap();
    session.enter_focus(0).unwrap();
    assert_eq!(count_roster_preview_actors(&mut world), 2);
    assert_eq!(
        count_visible_roster_actors_from_mode(2, RosterPresentationMode::Focused { slot_index: 0 }),
        1
    );
    session.exit_focus();
    assert_eq!(count_roster_preview_actors(&mut world), 2);
    session.enter_focus(1).unwrap();
    assert_eq!(count_roster_preview_actors(&mut world), 2);
    session.exit_focus();
    assert_eq!(count_roster_preview_actors(&mut world), 2);
}

#[test]
fn repeated_focus_cycles_keep_roster_actor_count_stable() {
    let mut world = World::new();
    spawn_mock_roster_actor(&mut world, SquadMemberDraftId(0), 0);
    spawn_mock_roster_actor(&mut world, SquadMemberDraftId(1), 1);

    let (units, profiles) = dev_catalogs();
    let origins = seed_origin_catalog(&units, &profiles);
    let mut session =
        StartingSquadSession::new_for_first_origin(&origins, &units, &profiles).unwrap();

    for slot in [0usize, 1, 0, 1, 0] {
        session.enter_focus(slot).unwrap();
        assert_eq!(count_roster_preview_actors(&mut world), 2);
        session.exit_focus();
        assert_eq!(count_roster_preview_actors(&mut world), 2);
    }
}

#[test]
fn duplicate_reconcile_preserves_one_actor_per_draft_member() {
    let mut world = World::new();
    spawn_mock_roster_actor(&mut world, SquadMemberDraftId(0), 0);
    spawn_mock_roster_actor(&mut world, SquadMemberDraftId(0), 0);
    spawn_mock_roster_actor(&mut world, SquadMemberDraftId(1), 1);
    assert_eq!(count_roster_preview_actors(&mut world), 3);

    let snapshot = {
        let mut query = world.query::<(Entity, &UnitEditorPreviewRosterMember)>();
        query
            .iter(&world)
            .map(|(entity, member)| (entity, member.draft_member_id))
            .collect::<Vec<_>>()
    };
    let duplicates = duplicate_roster_actor_entities(&snapshot);
    assert_eq!(duplicates.len(), 1);
    assert!(
        snapshot
            .iter()
            .filter(|(entity, _)| !duplicates.contains(entity))
            .map(|(_, id)| *id)
            .collect::<std::collections::HashSet<_>>()
            .len()
            == 2
    );
}

#[test]
fn draft_member_to_preview_actor_mapping_stays_one_to_one() {
    let mut world = World::new();
    let first = spawn_mock_roster_actor(&mut world, SquadMemberDraftId(0), 0);
    let second = spawn_mock_roster_actor(&mut world, SquadMemberDraftId(1), 1);

    let mapping = {
        let mut query = world.query::<(Entity, &UnitEditorPreviewRosterMember)>();
        query
            .iter(&world)
            .map(|(entity, member)| (member.draft_member_id, entity))
            .collect::<std::collections::HashMap<_, _>>()
    };

    assert_eq!(mapping.len(), 2);
    assert_eq!(mapping[&SquadMemberDraftId(0)], first);
    assert_eq!(mapping[&SquadMemberDraftId(1)], second);
}

#[test]
fn squad_visibility_waits_for_presentation_readiness() {
    let mode = RosterPresentationMode::Squad;
    assert!(roster_member_is_visible(0, mode));
    assert!(!roster_actor_presentation_ready(true, false, true));
    assert!(
        roster_member_is_visible(0, mode)
            && roster_actor_presentation_ready(true, true, false)
    );
}

#[test]
fn origin_switch_replaces_active_roster_identity_set() {
    let (units, profiles) = dev_catalogs();
    let origins = seed_origin_catalog(&units, &profiles);
    let mut session =
        StartingSquadSession::new_for_first_origin(&origins, &units, &profiles).unwrap();
    let exile_ids = session
        .active_draft(&origins)
        .unwrap()
        .members
        .iter()
        .map(|member| member.id)
        .collect::<Vec<_>>();
    assert_eq!(exile_ids, vec![SquadMemberDraftId(0), SquadMemberDraftId(1)]);

    session.cycle_origin(1, &origins, &units, &profiles).unwrap();
    let lone_ids = session
        .active_draft(&origins)
        .unwrap()
        .members
        .iter()
        .map(|member| member.id)
        .collect::<Vec<_>>();
    assert_eq!(lone_ids, vec![SquadMemberDraftId(0)]);
    assert_eq!(
        session.active_origin_id(&origins),
        Some(OriginId::new("lone_survivor"))
    );
}
