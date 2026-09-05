//! Settlement Workforce matrix focused tests.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use crate::client::CameraSettlementContext;
use crate::ui::gameplay::build_mode::BuildModeState;
use crate::ui::gameplay::floating_window::{
    FloatingGameplayWindowId, FloatingGameplayWindowRegistry, FloatingGameplayWindowRoot,
    FloatingWindowTitleBarDragRegion,
};
use crate::ui::gameplay::settlement_workforce::{
    NO_FOCUSED_SETTLEMENT_MESSAGE, SettlementWorkforcePanelState,
    SettlementWorkforcePanelTitleText, build_settlement_workforce_snapshot,
    collect_settlement_workforce_keyboard_input, permission_column_labels,
    settlement_workforce_member_unit_ids, snapshot_contains_permission_column,
    spawn_settlement_workforce_panel, sync_settlement_workforce_panel,
    sync_settlement_workforce_panel_visibility,
};
use crate::ui::gameplay::unit_skills::panel_contains_workforce_permission_controls;
use crate::world::{
    ChunkCoord, ChunkData, ChunkLayout, Heightfield, LocalPosition, SettlementId, SettlementKind,
    SettlementOwnership, UnitCatalog, UnitDefinitionId, UnitId, UnitOwnership, UnitSource,
    UnitState, WorkPermissionDomain, WorkSkillId, WorldConfig, WorldData, WorldPosition,
    assign_unit_settlement, create_settlement, create_unit_with_ownership,
    deny_all_unit_work_permissions, set_unit_work_permission, set_work_skill_value,
    settlement_member_unit_ids, unit_work_allowed,
};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::ui::UiPlugin));
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<WorldConfig>()
        .init_resource::<WorldData>()
        .init_resource::<UnitCatalog>()
        .init_resource::<crate::world::WorkSkillCatalog>()
        .init_resource::<CameraSettlementContext>()
        .init_resource::<SettlementWorkforcePanelState>()
        .init_resource::<BuildModeState>()
        .init_resource::<FloatingGameplayWindowRegistry>();
    app
}

fn flat_world() -> WorldData {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
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

fn bandit_catalog() -> UnitCatalog {
    UnitCatalog::from_definitions(crate::world::starter_unit_definitions()).unwrap()
}

fn settlement_with_members_at(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    settlement_pos: WorldPosition,
    name: &str,
    count: usize,
) -> (SettlementId, Vec<UnitId>) {
    let settlement_id = create_settlement(
        world,
        settlement_pos,
        name,
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap()
    .settlement_id;
    let mut members = Vec::new();
    for index in 0..count {
        let unit_id = create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("bandit"),
            WorldPosition::new(
                settlement_pos.chunk,
                LocalPosition::new(Vec3::new(
                    settlement_pos.local.0.x - 4.0 + index as f32,
                    settlement_pos.local.0.y,
                    settlement_pos.local.0.z,
                )),
            ),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        assign_unit_settlement(world, unit_id, Some(settlement_id)).unwrap();
        members.push(unit_id);
    }
    members.sort_by_key(|id| id.raw());
    (settlement_id, members)
}

fn settlement_with_members(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    count: usize,
) -> (SettlementId, Vec<UnitId>) {
    settlement_with_members_at(world, catalog, pos(64.0, 64.0), "Settlement 1", count)
}

#[test]
fn snapshot_resolves_settlement_from_camera_context() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 2);
    let context = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let snapshot = build_settlement_workforce_snapshot(
        &context,
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    assert_eq!(snapshot.settlement_id, Some(settlement_id));
    assert_eq!(snapshot.rows.len(), members.len());
    assert!(snapshot.title.contains("Settlement 1"));
}

#[test]
fn no_context_shows_no_stale_settlement() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, _) = settlement_with_members(&mut world, &catalog, 1);
    let context = CameraSettlementContext::default();
    let snapshot = build_settlement_workforce_snapshot(
        &context,
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    assert_eq!(snapshot.settlement_id, None);
    assert_eq!(
        snapshot.empty_message.as_deref(),
        Some(NO_FOCUSED_SETTLEMENT_MESSAGE)
    );
    assert!(snapshot.rows.is_empty());
    assert!(!snapshot.title.contains("Settlement 1"));

    let focused = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let focused_snapshot = build_settlement_workforce_snapshot(
        &focused,
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    assert_eq!(focused_snapshot.rows.len(), 1);
}

#[test]
fn rows_contain_only_authoritative_settlement_members() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 2);
    let outsider = create_unit_with_ownership(
        &catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(10.0, 10.0),
        UnitSource::Authored,
        UnitOwnership::player_default(),
    )
    .unwrap()
    .id;
    let context = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let snapshot = build_settlement_workforce_snapshot(
        &context,
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    let row_ids = snapshot
        .rows
        .iter()
        .map(|row| row.unit_id)
        .collect::<Vec<_>>();
    assert_eq!(row_ids, members);
    assert!(!row_ids.contains(&outsider));
}

#[test]
fn removed_dead_and_non_member_units_do_not_remain_in_matrix() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 3);
    let removed = members[0];
    let dead = members[1];
    world.remove_unit_by_id(removed);
    world.set_unit_state(dead, UnitState::Dead).unwrap();
    let context = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let snapshot = build_settlement_workforce_snapshot(
        &context,
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    assert_eq!(snapshot.rows.len(), 1);
    assert_eq!(snapshot.rows[0].unit_id, members[2]);
}

#[test]
fn skill_columns_use_expected_work_skills() {
    let catalog = bandit_catalog();
    let work_skills = crate::world::WorkSkillCatalog::default();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 1);
    let worker = members[0];
    set_work_skill_value(
        &mut world,
        &work_skills,
        worker,
        &WorkSkillId::new("farming"),
        12,
    )
    .unwrap();
    set_work_skill_value(
        &mut world,
        &work_skills,
        worker,
        &WorkSkillId::new("general_labor"),
        89,
    )
    .unwrap();
    set_work_skill_value(
        &mut world,
        &work_skills,
        worker,
        &WorkSkillId::new("construction"),
        45,
    )
    .unwrap();
    let context = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let snapshot = build_settlement_workforce_snapshot(&context, &world, &catalog, &work_skills);
    let row = &snapshot.rows[0];
    assert_eq!(row.cells.len(), 6);
    assert_eq!(row.cells[0].skill_value, 12);
    assert_eq!(row.cells[1].skill_value, 89);
    assert_eq!(row.cells[2].skill_value, 45);
    assert_eq!(row.cells[1].domain, WorkPermissionDomain::GeneralLabor);
}

#[test]
fn permission_columns_include_all_six_work_categories() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, _) = settlement_with_members(&mut world, &catalog, 1);
    let context = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let snapshot = build_settlement_workforce_snapshot(
        &context,
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    let labels = permission_column_labels(&snapshot);
    assert_eq!(
        labels,
        vec![
            "Farming",
            "General Labor",
            "Construction",
            "Cooking",
            "Science",
            "Smithing",
        ]
    );
    for domain in WorkPermissionDomain::ALL {
        assert!(snapshot_contains_permission_column(&snapshot, domain));
    }
}

#[test]
fn default_permission_displays_checked() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 1);
    let context = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let snapshot = build_settlement_workforce_snapshot(
        &context,
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    assert!(
        snapshot.rows[0]
            .cells
            .iter()
            .all(|cell| cell.permission_allowed)
    );
    assert!(unit_work_allowed(
        &world,
        settlement_id,
        members[0],
        WorkPermissionDomain::Farming
    ));
}

#[test]
fn unchecking_farming_writes_through_authoritative_api() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 1);
    let worker = members[0];
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::Farming,
        false,
    )
    .unwrap();
    assert!(!unit_work_allowed(
        &world,
        settlement_id,
        worker,
        WorkPermissionDomain::Farming
    ));
}

#[test]
fn general_labor_deny_blocks_general_labor_only() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 1);
    let worker = members[0];
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::GeneralLabor,
        false,
    )
    .unwrap();
    assert!(!unit_work_allowed(
        &world,
        settlement_id,
        worker,
        WorkPermissionDomain::GeneralLabor
    ));
    assert!(unit_work_allowed(
        &world,
        settlement_id,
        worker,
        WorkPermissionDomain::Farming
    ));
}

#[test]
fn clear_all_disables_all_permission_domains() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 1);
    let worker = members[0];
    deny_all_unit_work_permissions(&mut world, settlement_id, worker).unwrap();
    for domain in WorkPermissionDomain::ALL {
        assert!(!unit_work_allowed(&world, settlement_id, worker, domain));
    }
}

#[test]
fn permission_changes_do_not_mutate_skill_values() {
    let catalog = bandit_catalog();
    let work_skills = crate::world::WorkSkillCatalog::default();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 1);
    let worker = members[0];
    set_work_skill_value(
        &mut world,
        &work_skills,
        worker,
        &WorkSkillId::new("farming"),
        77,
    )
    .unwrap();
    deny_all_unit_work_permissions(&mut world, settlement_id, worker).unwrap();
    let context = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let snapshot = build_settlement_workforce_snapshot(&context, &world, &catalog, &work_skills);
    assert_eq!(snapshot.rows[0].cells[0].skill_value, 77);
}

#[test]
fn skill_value_does_not_force_permission_state() {
    let catalog = bandit_catalog();
    let work_skills = crate::world::WorkSkillCatalog::default();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 1);
    let worker = members[0];
    set_work_skill_value(
        &mut world,
        &work_skills,
        worker,
        &WorkSkillId::new("farming"),
        99,
    )
    .unwrap();
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::Farming,
        false,
    )
    .unwrap();
    set_work_skill_value(
        &mut world,
        &work_skills,
        worker,
        &WorkSkillId::new("general_labor"),
        1,
    )
    .unwrap();
    let context = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let snapshot = build_settlement_workforce_snapshot(&context, &world, &catalog, &work_skills);
    assert!(!snapshot.rows[0].cells[0].permission_allowed);
    assert!(snapshot.rows[0].cells[1].permission_allowed);
}

#[test]
fn focused_settlement_change_rebuilds_membership() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_a, members_a) = settlement_with_members(&mut world, &catalog, 1);
    let (settlement_b, members_b) =
        settlement_with_members_at(&mut world, &catalog, pos(200.0, 200.0), "Settlement 2", 2);
    let first = build_settlement_workforce_snapshot(
        &CameraSettlementContext {
            focused_settlement_id: Some(settlement_a),
            focus_world_position: None,
        },
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    let second = build_settlement_workforce_snapshot(
        &CameraSettlementContext {
            focused_settlement_id: Some(settlement_b),
            focus_world_position: None,
        },
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    assert_eq!(first.rows[0].unit_id, members_a[0]);
    assert_eq!(second.rows.len(), members_b.len());
}

#[test]
fn skill_mutation_updates_displayed_matrix_value() {
    let catalog = bandit_catalog();
    let work_skills = crate::world::WorkSkillCatalog::default();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 1);
    let worker = members[0];
    let context = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let before = build_settlement_workforce_snapshot(&context, &world, &catalog, &work_skills);
    set_work_skill_value(
        &mut world,
        &work_skills,
        worker,
        &WorkSkillId::new("construction"),
        33,
    )
    .unwrap();
    let after = build_settlement_workforce_snapshot(&context, &world, &catalog, &work_skills);
    assert_ne!(
        before.rows[0].cells[2].skill_value,
        after.rows[0].cells[2].skill_value
    );
    assert_eq!(after.rows[0].cells[2].skill_value, 33);
}

#[test]
fn permission_mutation_updates_checkbox_state() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 1);
    let worker = members[0];
    let context = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let before = build_settlement_workforce_snapshot(
        &context,
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    set_unit_work_permission(
        &mut world,
        settlement_id,
        worker,
        WorkPermissionDomain::GeneralLabor,
        false,
    )
    .unwrap();
    let after = build_settlement_workforce_snapshot(
        &context,
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    let general_labor_index = after
        .permission_columns
        .iter()
        .position(|domain| *domain == WorkPermissionDomain::GeneralLabor)
        .expect("general labor column");
    assert!(before.rows[0].cells[general_labor_index].permission_allowed);
    assert!(!after.rows[0].cells[general_labor_index].permission_allowed);
}

#[test]
fn panel_uses_floating_gameplay_window_shell() {
    let mut app = headless_app();
    app.world_mut()
        .run_system_once(spawn_settlement_workforce_panel)
        .expect("spawn");
    let mut world = app.world_mut();
    let roots: Vec<_> = world
        .query::<&FloatingGameplayWindowRoot>()
        .iter(&mut world)
        .filter(|root| root.id == FloatingGameplayWindowId::SettlementWorkforce)
        .collect();
    assert_eq!(roots.len(), 1);
    let drag_regions: Vec<_> = world
        .query::<&FloatingWindowTitleBarDragRegion>()
        .iter(&mut world)
        .filter(|region| region.id == FloatingGameplayWindowId::SettlementWorkforce)
        .collect();
    assert_eq!(drag_regions.len(), 1);
}

#[test]
fn member_listing_is_deterministically_sorted() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, _) = settlement_with_members(&mut world, &catalog, 3);
    let ids = settlement_workforce_member_unit_ids(&world, settlement_id);
    assert!(ids.windows(2).all(|pair| pair[0].raw() <= pair[1].raw()));
}

#[test]
fn sync_rebuilds_matrix_when_open() {
    let mut app = headless_app();
    app.world_mut()
        .run_system_once(spawn_settlement_workforce_panel)
        .expect("spawn");
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, _) = settlement_with_members(&mut world, &catalog, 1);
    *app.world_mut().resource_mut::<WorldData>() = world;
    *app.world_mut().resource_mut::<UnitCatalog>() = catalog;
    *app.world_mut().resource_mut::<CameraSettlementContext>() = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    app.world_mut()
        .resource_mut::<SettlementWorkforcePanelState>()
        .open_panel();
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel_visibility)
        .expect("visibility");
    let snapshot = build_settlement_workforce_snapshot(
        app.world().resource::<CameraSettlementContext>(),
        app.world().resource::<WorldData>(),
        app.world().resource::<UnitCatalog>(),
        app.world().resource::<crate::world::WorkSkillCatalog>(),
    );
    assert_eq!(snapshot.rows.len(), 1);
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("sync");
    let title = {
        let mut world = app.world_mut();
        world
            .query::<&Text>()
            .iter(&mut world)
            .map(|text| text.to_string())
            .find(|text| text.contains("Settlement Workforce — Settlement 1"))
            .expect("title")
    };
    assert!(title.contains("Settlement 1"));
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("cached sync");
}

#[test]
fn u_screen_does_not_gain_workforce_permission_controls() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (_, members) = settlement_with_members(&mut world, &catalog, 1);
    let snapshot = crate::ui::gameplay::unit_skills::build_unit_skills_snapshot(
        members[0],
        &world,
        &catalog,
        &crate::world::WeaponCatalog::default(),
        &crate::world::WorkSkillCatalog::default(),
    )
    .unwrap();
    let text = crate::ui::gameplay::unit_skills::format_unit_skills_panel_text(&snapshot);
    assert!(!panel_contains_workforce_permission_controls(&text));
}

#[test]
fn n_opens_settlement_workforce_panel_when_closed() {
    let mut app = headless_app();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyN);
    app.world_mut()
        .run_system_once(collect_settlement_workforce_keyboard_input)
        .expect("input");
    assert!(app.world().resource::<SettlementWorkforcePanelState>().open);
}

#[test]
fn n_closes_settlement_workforce_panel_when_open() {
    let mut app = headless_app();
    app.world_mut()
        .resource_mut::<SettlementWorkforcePanelState>()
        .open_panel();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyN);
    app.world_mut()
        .run_system_once(collect_settlement_workforce_keyboard_input)
        .expect("input");
    assert!(!app.world().resource::<SettlementWorkforcePanelState>().open);
}

#[test]
fn n_opens_empty_state_without_focused_settlement() {
    let mut app = headless_app();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyN);
    app.world_mut()
        .run_system_once(collect_settlement_workforce_keyboard_input)
        .expect("input");
    assert!(app.world().resource::<SettlementWorkforcePanelState>().open);
    let snapshot = build_settlement_workforce_snapshot(
        app.world().resource::<CameraSettlementContext>(),
        app.world().resource::<WorldData>(),
        app.world().resource::<UnitCatalog>(),
        app.world().resource::<crate::world::WorkSkillCatalog>(),
    );
    assert_eq!(
        snapshot.empty_message.as_deref(),
        Some(NO_FOCUSED_SETTLEMENT_MESSAGE)
    );
}

#[test]
fn build_search_focus_blocks_n_hotkey() {
    let mut app = headless_app();
    app.world_mut()
        .resource_mut::<BuildModeState>()
        .search_focused = true;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyN);
    app.world_mut()
        .run_system_once(collect_settlement_workforce_keyboard_input)
        .expect("input");
    assert!(!app.world().resource::<SettlementWorkforcePanelState>().open);
}

#[test]
fn dev_assign_path_snapshot_survives_stale_membership_index() {
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 2);
    for member in &members {
        assign_unit_settlement(&mut world, *member, Some(settlement_id)).unwrap();
    }
    world.settlement_store_mut().clear_membership_indexes();
    assert_eq!(
        world
            .settlement_store()
            .units_for_settlement(settlement_id)
            .len(),
        0
    );
    assert_eq!(settlement_member_unit_ids(&world, settlement_id).len(), 2);
    let context = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    let snapshot = build_settlement_workforce_snapshot(
        &context,
        &world,
        &catalog,
        &crate::world::WorkSkillCatalog::default(),
    );
    assert_eq!(snapshot.rows.len(), 2);
    assert_eq!(
        snapshot
            .rows
            .iter()
            .map(|row| row.unit_id)
            .collect::<Vec<_>>(),
        members
    );
    for row in &snapshot.rows {
        assert_eq!(row.cells.len(), 6);
    }
}
