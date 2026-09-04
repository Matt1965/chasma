//! Unit Skills screen focused tests.

use bevy::ecs::system::RunSystemOnce;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::ui::gameplay::build_mode::BuildModeState;
use crate::ui::gameplay::floating_window::{
    FloatingGameplayWindowId, FloatingGameplayWindowRegistry, FloatingGameplayWindowRoot,
    FloatingWindowTitleBarDragRegion,
};
use crate::ui::gameplay::player_hud_state::primary_selected_unit;
use crate::ui::gameplay::unit_skills::{
    UnitSkillsPanelState, build_unit_skills_snapshot, collect_unit_skills_keyboard_input,
    format_unit_skills_panel_text, panel_contains_workforce_permission_controls,
    reconcile_unit_skills_panel, spawn_unit_skills_panel, sync_unit_skills_panel,
    sync_unit_skills_panel_visibility,
};
use crate::units::input::SelectedUnits;
use crate::world::{
    ChunkCoord, ChunkData, ChunkLayout, Heightfield, LocalPosition, UnitCatalog, UnitDefinitionId,
    UnitId, UnitSource, WeaponCatalog, WorldConfig, WorldData, WorldPosition, create_unit,
    starter_weapon_definitions,
};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::ui::UiPlugin));
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<WorldConfig>()
        .init_resource::<WorldData>()
        .init_resource::<SelectedUnits>()
        .init_resource::<UnitSkillsPanelState>()
        .init_resource::<BuildModeState>()
        .init_resource::<UnitCatalog>()
        .init_resource::<WeaponCatalog>()
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

fn wolf_catalog() -> UnitCatalog {
    UnitCatalog::from_definitions(crate::world::starter_unit_definitions()).unwrap()
}

fn weapons() -> WeaponCatalog {
    WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap()
}

fn spawn_unit(world: &mut WorldData, catalog: &UnitCatalog) -> UnitId {
    create_unit(
        catalog,
        world,
        &UnitDefinitionId::new("wolf"),
        pos(1.0, 1.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id
}

#[test]
fn u_opens_unit_skills_for_primary_selected_unit() {
    let mut app = headless_app();
    let catalog = wolf_catalog();
    let mut world = flat_world();
    let unit_id = spawn_unit(&mut world, &catalog);
    *app.world_mut().resource_mut::<WorldData>() = world;
    *app.world_mut().resource_mut::<UnitCatalog>() = catalog;
    app.world_mut()
        .resource_mut::<SelectedUnits>()
        .set_single(unit_id);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyU);
    app.world_mut()
        .run_system_once(collect_unit_skills_keyboard_input)
        .expect("input");
    let panel = app.world().resource::<UnitSkillsPanelState>();
    assert!(panel.open);
    assert_eq!(panel.displayed_unit_id, Some(unit_id));
}

#[test]
fn multiple_selection_uses_lowest_unit_id_primary() {
    let mut selection = SelectedUnits::default();
    selection.replace_with([UnitId::new(9), UnitId::new(2)]);
    assert_eq!(primary_selected_unit(&selection), Some(UnitId::new(2)));
}

#[test]
fn no_unit_does_not_open_stale_panel() {
    let mut app = headless_app();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyU);
    app.world_mut()
        .run_system_once(collect_unit_skills_keyboard_input)
        .expect("input");
    assert!(!app.world().resource::<UnitSkillsPanelState>().open);
}

#[test]
fn u_toggle_closes_open_panel_without_selection() {
    let mut app = headless_app();
    {
        let mut panel = app.world_mut().resource_mut::<UnitSkillsPanelState>();
        panel.open_for(UnitId::new(1));
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyU);
    app.world_mut()
        .run_system_once(collect_unit_skills_keyboard_input)
        .expect("input");
    assert!(!app.world().resource::<UnitSkillsPanelState>().open);
}

#[test]
fn build_search_focus_blocks_u_hotkey() {
    let mut app = headless_app();
    let catalog = wolf_catalog();
    let mut world = flat_world();
    let unit_id = spawn_unit(&mut world, &catalog);
    *app.world_mut().resource_mut::<WorldData>() = world;
    app.world_mut()
        .resource_mut::<SelectedUnits>()
        .set_single(unit_id);
    app.world_mut()
        .resource_mut::<BuildModeState>()
        .search_focused = true;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyU);
    app.world_mut()
        .run_system_once(collect_unit_skills_keyboard_input)
        .expect("input");
    assert!(!app.world().resource::<UnitSkillsPanelState>().open);
}

#[test]
fn selection_change_updates_displayed_unit() {
    let mut app = headless_app();
    let catalog = wolf_catalog();
    let mut world = flat_world();
    let first = spawn_unit(&mut world, &catalog);
    let second = create_unit(
        &catalog,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(2.0, 2.0),
        UnitSource::Authored,
    )
    .unwrap()
    .id;
    *app.world_mut().resource_mut::<WorldData>() = world;
    *app.world_mut().resource_mut::<UnitCatalog>() = catalog;
    *app.world_mut().resource_mut::<WeaponCatalog>() = weapons();
    {
        let mut panel = app.world_mut().resource_mut::<UnitSkillsPanelState>();
        panel.open_for(first);
    }
    app.world_mut()
        .resource_mut::<SelectedUnits>()
        .replace_with([second, first]);
    app.world_mut()
        .run_system_once(reconcile_unit_skills_panel)
        .expect("reconcile");
    assert_eq!(
        app.world()
            .resource::<UnitSkillsPanelState>()
            .displayed_unit_id,
        Some(first)
    );
}

#[test]
fn removed_unit_closes_panel_on_reconcile() {
    let mut app = headless_app();
    let catalog = wolf_catalog();
    let mut world = flat_world();
    let unit_id = spawn_unit(&mut world, &catalog);
    world.remove_unit_by_id(unit_id);
    *app.world_mut().resource_mut::<WorldData>() = world;
    {
        let mut panel = app.world_mut().resource_mut::<UnitSkillsPanelState>();
        panel.open_for(unit_id);
    }
    app.world_mut()
        .run_system_once(reconcile_unit_skills_panel)
        .expect("reconcile");
    assert!(!app.world().resource::<UnitSkillsPanelState>().open);
}

#[test]
fn panel_uses_floating_gameplay_window_shell() {
    let mut app = headless_app();
    app.world_mut()
        .run_system_once(spawn_unit_skills_panel)
        .expect("spawn");
    let mut world = app.world_mut();
    let roots: Vec<_> = world
        .query::<&FloatingGameplayWindowRoot>()
        .iter(&mut world)
        .filter(|root| root.id == FloatingGameplayWindowId::UnitSkills)
        .collect();
    assert_eq!(roots.len(), 1);
    let drag_regions: Vec<_> = world
        .query::<&FloatingWindowTitleBarDragRegion>()
        .iter(&mut world)
        .filter(|region| region.id == FloatingGameplayWindowId::UnitSkills)
        .collect();
    assert_eq!(drag_regions.len(), 1);
}

#[test]
fn sync_updates_body_text_for_open_panel() {
    let mut app = headless_app();
    app.world_mut()
        .run_system_once(spawn_unit_skills_panel)
        .expect("spawn");
    let catalog = wolf_catalog();
    let mut world = flat_world();
    let unit_id = spawn_unit(&mut world, &catalog);
    *app.world_mut().resource_mut::<WorldData>() = world;
    *app.world_mut().resource_mut::<UnitCatalog>() = catalog.clone();
    *app.world_mut().resource_mut::<WeaponCatalog>() = weapons();
    {
        let mut panel = app.world_mut().resource_mut::<UnitSkillsPanelState>();
        panel.open_for(unit_id);
    }
    app.world_mut()
        .run_system_once(sync_unit_skills_panel_visibility)
        .expect("visibility");
    app.world_mut()
        .run_system_once(sync_unit_skills_panel)
        .expect("sync");
    let snapshot = build_unit_skills_snapshot(
        unit_id,
        app.world().resource::<WorldData>(),
        &catalog,
        &weapons(),
    )
    .unwrap();
    let expected = format_unit_skills_panel_text(&snapshot);
    let mut world = app.world_mut();
    let body = world
        .query::<&Text>()
        .iter(&mut world)
        .map(|text| text.to_string())
        .find(|text| text.contains("Strength:"))
        .expect("body");
    assert_eq!(body, expected);
    assert!(!panel_contains_workforce_permission_controls(&body));
}
