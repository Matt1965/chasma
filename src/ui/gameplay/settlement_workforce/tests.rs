//! Settlement Workforce matrix focused tests.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::ui::OverflowAxis;

use crate::client::CameraSettlementContext;
use crate::ui::gameplay::build_mode::BuildModeState;
use crate::ui::gameplay::floating_window::{
    FloatingGameplayWindowId, FloatingGameplayWindowRegistry, FloatingGameplayWindowRoot,
    FloatingWindowTitleBarDragRegion,
};
use crate::ui::gameplay::settlement_workforce::{
    CLOSE_BUTTON_LABEL, MATRIX_COLUMN_COUNT, NO_FOCUSED_SETTLEMENT_MESSAGE, PANEL_MIN_WIDTH_PX,
    PANEL_WIDTH_PX, SettlementWorkforceMatrixBody, SettlementWorkforceMatrixContentHost,
    SettlementWorkforceMatrixDataRow, SettlementWorkforceMatrixHeaderHost,
    SettlementWorkforceMatrixHeaderRow, SettlementWorkforceMatrixHorizontalScroll,
    SettlementWorkforceMatrixRowsScroll, SettlementWorkforcePanelCloseButton,
    SettlementWorkforcePanelRoot, SettlementWorkforcePanelState, SettlementWorkforcePanelTitleText,
    SettlementWorkforceScrollPlugin, SettlementWorkforceScrollState,
    SettlementWorkforceVerticalScrollbar, WorkforceAllowAllButton, WorkforcePermissionCheckbox,
    build_settlement_workforce_snapshot, clamp_scroll_offset_y,
    collect_settlement_workforce_keyboard_input, forbidden_workforce_ui_characters,
    matrix_min_width, max_scroll_y, permission_checkbox_label, permission_column_labels,
    settlement_workforce_member_unit_ids, snapshot_contains_permission_column,
    spawn_settlement_workforce_panel, sync_settlement_workforce_panel,
    sync_settlement_workforce_panel_dimensions, sync_settlement_workforce_panel_visibility,
};
use crate::ui::gameplay::text::{format_ui_title, ui_chrome_contains_forbidden_glyph};
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
    app.add_plugins(SettlementWorkforceScrollPlugin)
        .init_resource::<ButtonInput<KeyCode>>()
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

fn count_matrix_data_rows(world: &mut World) -> usize {
    world
        .query::<&SettlementWorkforceMatrixDataRow>()
        .iter(world)
        .count()
}

fn count_permission_checkboxes(world: &mut World) -> usize {
    world
        .query::<&WorkforcePermissionCheckbox>()
        .iter(world)
        .count()
}

fn panel_root_min_height(world: &mut World) -> Option<f32> {
    world
        .query_filtered::<&Node, With<SettlementWorkforcePanelRoot>>()
        .iter(world)
        .next()
        .and_then(|node| match node.min_height {
            Val::Px(value) => Some(value),
            _ => None,
        })
}

fn panel_root_height_px(world: &mut World) -> Option<f32> {
    world
        .query_filtered::<&Node, With<SettlementWorkforcePanelRoot>>()
        .iter(world)
        .next()
        .and_then(|node| match node.height {
            Val::Px(value) => Some(value),
            _ => None,
        })
}

fn vertical_scrollbar_count(world: &mut World) -> usize {
    world
        .query::<&SettlementWorkforceVerticalScrollbar>()
        .iter(world)
        .count()
}

fn panel_root_max_height_percent(world: &mut World) -> Option<f32> {
    world
        .query_filtered::<&Node, With<SettlementWorkforcePanelRoot>>()
        .iter(world)
        .next()
        .and_then(|node| match node.max_height {
            Val::Percent(value) => Some(value),
            _ => None,
        })
}

fn rows_scroll_uses_vertical_overflow(world: &mut World) -> bool {
    world
        .query_filtered::<&Node, With<SettlementWorkforceMatrixRowsScroll>>()
        .iter(world)
        .any(|node| node.overflow.y == OverflowAxis::Scroll)
}

fn horizontal_scroll_uses_x_overflow(world: &mut World) -> bool {
    world
        .query_filtered::<&Node, With<SettlementWorkforceMatrixHorizontalScroll>>()
        .iter(world)
        .any(|node| node.overflow.x == OverflowAxis::Scroll)
}

fn header_column_count(world: &mut World) -> usize {
    world
        .query_filtered::<&Children, With<SettlementWorkforceMatrixHeaderRow>>()
        .iter(world)
        .map(|children| children.len())
        .next()
        .unwrap_or(0)
}

fn data_row_child_counts(world: &mut World) -> Vec<usize> {
    world
        .query_filtered::<&Children, With<SettlementWorkforceMatrixDataRow>>()
        .iter(world)
        .map(|children| children.len())
        .collect()
}

fn workforce_ui_text_values(world: &mut World) -> Vec<String> {
    world
        .query::<&Text>()
        .iter(world)
        .map(|text| text.to_string())
        .collect()
}

fn workforce_close_button_labels(world: &mut World) -> Vec<String> {
    let mut labels = Vec::new();
    for (children, _) in world
        .query::<(&Children, &SettlementWorkforcePanelCloseButton)>()
        .iter(world)
    {
        for child in children.iter() {
            if let Some(text) = world.get::<Text>(child) {
                labels.push(text.to_string());
            }
        }
    }
    labels
}

fn open_panel_with_members(app: &mut App, member_count: usize) -> (SettlementId, Vec<UnitId>) {
    app.world_mut()
        .run_system_once(spawn_settlement_workforce_panel)
        .expect("spawn");
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, member_count);
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
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel_dimensions)
        .expect("dimensions");
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("sync");
    (settlement_id, members)
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
    let mut world = app.world_mut();
    assert_eq!(count_matrix_data_rows(&mut world), 1);
    assert_eq!(count_permission_checkboxes(&mut world), 6);
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

#[test]
fn open_panel_spawns_matrix_row_entities_for_two_members() {
    let mut app = headless_app();
    app.world_mut()
        .run_system_once(spawn_settlement_workforce_panel)
        .expect("spawn");
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 2);
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
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("sync");
    let mut world = app.world_mut();
    assert_eq!(count_matrix_data_rows(&mut world), 2);
    assert_eq!(count_permission_checkboxes(&mut world), 12);
    let row_ids = world
        .query::<&SettlementWorkforceMatrixDataRow>()
        .iter(&mut world)
        .map(|row| row.unit_id)
        .collect::<Vec<_>>();
    assert_eq!(row_ids, members);
}

#[test]
fn matrix_scroll_viewport_and_content_host_exist_when_open() {
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
        .run_system_once(sync_settlement_workforce_panel)
        .expect("sync");
    let mut world = app.world_mut();
    assert_eq!(
        world
            .query::<&SettlementWorkforceMatrixBody>()
            .iter(&mut world)
            .count(),
        1
    );
    assert_eq!(
        world
            .query::<&SettlementWorkforceMatrixContentHost>()
            .iter(&mut world)
            .count(),
        1
    );
    assert_eq!(
        world
            .query::<&SettlementWorkforceMatrixHeaderHost>()
            .iter(&mut world)
            .count(),
        1
    );
    assert!(panel_root_min_height(&mut world).is_some());
    assert!(rows_scroll_uses_vertical_overflow(&mut world));
    assert!(horizontal_scroll_uses_x_overflow(&mut world));
    assert_eq!(header_column_count(&mut world), MATRIX_COLUMN_COUNT);
}

#[test]
fn reopen_after_close_materializes_rows_even_when_snapshot_unchanged() {
    let mut app = headless_app();
    app.world_mut()
        .run_system_once(spawn_settlement_workforce_panel)
        .expect("spawn");
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, _) = settlement_with_members(&mut world, &catalog, 2);
    *app.world_mut().resource_mut::<WorldData>() = world;
    *app.world_mut().resource_mut::<UnitCatalog>() = catalog;
    *app.world_mut().resource_mut::<CameraSettlementContext>() = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    {
        let mut panel = app
            .world_mut()
            .resource_mut::<SettlementWorkforcePanelState>();
        panel.open_panel();
    }
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("first sync");
    {
        let mut panel = app
            .world_mut()
            .resource_mut::<SettlementWorkforcePanelState>();
        panel.close();
    }
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("close sync");
    {
        let mut panel = app
            .world_mut()
            .resource_mut::<SettlementWorkforcePanelState>();
        panel.open_panel();
    }
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("reopen sync");
    let mut world = app.world_mut();
    assert_eq!(count_matrix_data_rows(&mut world), 2);
}

#[test]
fn focus_change_while_open_rebuilds_rows() {
    let mut app = headless_app();
    app.world_mut()
        .run_system_once(spawn_settlement_workforce_panel)
        .expect("spawn");
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, _) = settlement_with_members(&mut world, &catalog, 2);
    *app.world_mut().resource_mut::<WorldData>() = world;
    *app.world_mut().resource_mut::<UnitCatalog>() = catalog;
    app.world_mut()
        .resource_mut::<SettlementWorkforcePanelState>()
        .open_panel();
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("no focus");
    let mut world = app.world_mut();
    assert_eq!(count_matrix_data_rows(&mut world), 0);
    *app.world_mut().resource_mut::<CameraSettlementContext>() = CameraSettlementContext {
        focused_settlement_id: Some(settlement_id),
        focus_world_position: None,
    };
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("focused");
    let mut world = app.world_mut();
    assert_eq!(count_matrix_data_rows(&mut world), 2);
    *app.world_mut().resource_mut::<CameraSettlementContext>() = CameraSettlementContext::default();
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("lost focus");
    let mut world = app.world_mut();
    assert_eq!(count_matrix_data_rows(&mut world), 0);
}

#[test]
fn membership_change_while_open_rebuilds_row_count() {
    let mut app = headless_app();
    app.world_mut()
        .run_system_once(spawn_settlement_workforce_panel)
        .expect("spawn");
    let catalog = bandit_catalog();
    let mut world = flat_world();
    let (settlement_id, members) = settlement_with_members(&mut world, &catalog, 1);
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
        .run_system_once(sync_settlement_workforce_panel)
        .expect("one row");
    {
        let mut world = app.world_mut();
        assert_eq!(count_matrix_data_rows(&mut world), 1);
    }
    {
        let catalog = bandit_catalog();
        let mut world = app.world_mut().resource_mut::<WorldData>();
        let extra = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(62.0, 64.0),
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        assign_unit_settlement(&mut world, extra, Some(settlement_id)).unwrap();
    }
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("two rows");
    {
        let mut world = app.world_mut();
        assert_eq!(count_matrix_data_rows(&mut world), 2);
    }
    {
        let mut world = app.world_mut().resource_mut::<WorldData>();
        assign_unit_settlement(&mut world, members[0], None).unwrap();
    }
    app.world_mut()
        .run_system_once(sync_settlement_workforce_panel)
        .expect("one row again");
    let mut world = app.world_mut();
    assert_eq!(count_matrix_data_rows(&mut world), 1);
}

#[test]
fn permission_checkbox_labels_use_ascii_only() {
    assert_eq!(permission_checkbox_label(true), "[X]");
    assert_eq!(permission_checkbox_label(false), "[ ]");
    assert_eq!(CLOSE_BUTTON_LABEL, "X");
    for forbidden in forbidden_workforce_ui_characters() {
        assert!(!permission_checkbox_label(true).contains(*forbidden));
        assert!(!permission_checkbox_label(false).contains(*forbidden));
        assert!(!CLOSE_BUTTON_LABEL.contains(*forbidden));
    }
}

#[test]
fn synced_workforce_controls_contain_no_forbidden_glyphs() {
    let mut app = headless_app();
    let (_, _) = open_panel_with_members(&mut app, 2);
    let mut world = app.world_mut();
    let forbidden = forbidden_workforce_ui_characters();
    for value in workforce_ui_text_values(&mut world) {
        assert!(
            !forbidden.iter().any(|ch| value.contains(*ch)),
            "forbidden glyph in workforce UI text: {value}"
        );
    }
    let close_labels = workforce_close_button_labels(&mut world);
    assert_eq!(close_labels, vec![CLOSE_BUTTON_LABEL.to_string()]);
}

#[test]
fn header_and_rows_share_column_count() {
    let mut app = headless_app();
    let (_, _) = open_panel_with_members(&mut app, 2);
    let mut world = app.world_mut();
    assert_eq!(header_column_count(&mut world), MATRIX_COLUMN_COUNT);
    let row_child_counts = data_row_child_counts(&mut world);
    assert_eq!(row_child_counts.len(), 2);
    assert!(
        row_child_counts
            .iter()
            .all(|count| *count == MATRIX_COLUMN_COUNT)
    );
}

#[test]
fn smithing_and_row_controls_are_materialized() {
    let mut app = headless_app();
    let (_, _) = open_panel_with_members(&mut app, 2);
    let mut world = app.world_mut();
    let smithing_checkboxes = world
        .query::<&WorkforcePermissionCheckbox>()
        .iter(&mut world)
        .filter(|checkbox| checkbox.domain == WorkPermissionDomain::Smithing)
        .count();
    let allow_all_buttons = world
        .query::<&WorkforceAllowAllButton>()
        .iter(&mut world)
        .count();
    assert_eq!(smithing_checkboxes, 2);
    assert_eq!(allow_all_buttons, 2);
}

#[test]
fn twenty_five_workers_all_exist_without_unbounded_panel_height() {
    let mut app = headless_app();
    let (_, members) = open_panel_with_members(&mut app, 25);
    let mut world = app.world_mut();
    assert_eq!(count_matrix_data_rows(&mut world), 25);
    assert_eq!(count_permission_checkboxes(&mut world), 25 * 6);
    let row_ids = world
        .query::<&SettlementWorkforceMatrixDataRow>()
        .iter(&mut world)
        .map(|row| row.unit_id)
        .collect::<Vec<_>>();
    assert_eq!(row_ids, members);
    assert!(rows_scroll_uses_vertical_overflow(&mut world));
    assert_eq!(vertical_scrollbar_count(&mut world), 1);
    let root = world
        .query_filtered::<&Node, With<SettlementWorkforcePanelRoot>>()
        .iter(&mut world)
        .next()
        .expect("root");
    assert!(matches!(root.max_height, Val::Px(h) if h > 0.0));
    assert!(panel_root_height_px(&mut world).is_some());
}

#[test]
fn matrix_min_width_covers_all_columns() {
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
    assert_eq!(labels.len(), 6);
    assert!(matrix_min_width() >= PANEL_MIN_WIDTH_PX);
}

#[test]
fn scroll_math_reports_overflow_for_many_workers() {
    let viewport = 180.0;
    let content = 25.0 * 28.0;
    assert!(content > viewport);
    assert!(max_scroll_y(viewport, content) > 0.0);
    let mut state = SettlementWorkforceScrollState {
        scroll_offset_y: 0.0,
        viewport_height: viewport,
        content_height: content,
    };
    state.apply_wheel_delta(-4.0);
    assert!(state.scroll_offset_y > 0.0);
    state.scroll_offset_y = 999.0;
    state.clamp_offset();
    assert_eq!(state.scroll_offset_y, max_scroll_y(viewport, content));
}

#[test]
fn clamp_scroll_offset_y_matches_state_helper() {
    assert_eq!(clamp_scroll_offset_y(50.0, 100.0, 300.0), 50.0);
    assert_eq!(clamp_scroll_offset_y(500.0, 100.0, 300.0), 200.0);
}

#[test]
fn workforce_snapshot_title_uses_ascii_separator() {
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
    assert!(
        snapshot
            .title
            .contains("Settlement Workforce | Settlement 1")
    );
    assert!(!ui_chrome_contains_forbidden_glyph(&snapshot.title));
}

#[test]
fn vertical_scrollbar_entity_is_spawned() {
    let mut app = headless_app();
    app.world_mut()
        .run_system_once(spawn_settlement_workforce_panel)
        .expect("spawn");
    let mut world = app.world_mut();
    assert_eq!(vertical_scrollbar_count(&mut world), 1);
}

#[test]
fn panel_dimensions_sync_to_viewport_pixels() {
    let mut app = headless_app();
    let (_, _) = open_panel_with_members(&mut app, 2);
    let mut world = app.world_mut();
    let height = panel_root_height_px(&mut world);
    assert!(height.is_some_and(|value| value >= 240.0 && value <= 720.0 * 0.75));
}

#[test]
fn twenty_five_worker_scroll_state_can_reach_bottom() {
    let viewport = 200.0;
    let content = 25.0 * 28.0;
    let mut state = SettlementWorkforceScrollState {
        scroll_offset_y: 0.0,
        viewport_height: viewport,
        content_height: content,
    };
    state.scroll_offset_y = state.max_scroll_y();
    let (thumb_h, thumb_top) = state.thumb_metrics();
    assert!(thumb_h > 0.0);
    assert!(thumb_top > 0.0);
    assert_eq!(state.scroll_offset_y, max_scroll_y(viewport, content));
}
