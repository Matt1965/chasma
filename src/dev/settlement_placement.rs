//! Dev settlement anchor placement (ADR-133).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::debug::DebugOverlayCategory;
use crate::debug::DebugOverlaySettings;
use crate::dev::dev_mode::{DevModeInputGate, DevModeState};
use crate::dev::input::DevPanelHoverState;
use crate::dev::input::DevPanelUi;
use crate::dev::spawn_tools::dev_spawn_position_from_terrain_click;
use crate::player::selection_ring_mesh::{
    SELECTION_RING_SEGMENTS, draw_terrain_ring_gizmos, sample_terrain_ring_render_points,
};
use crate::simulation::SimulationControlState;
use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::units::input::{cursor_world_ray, terrain_click_to_world_position};
use crate::world::{
    CreateSettlementReport, DEFAULT_TOWN_BOUNDARY_RADIUS_METERS, SettlementCreationError,
    SettlementId, SettlementKind, SettlementOwnership, WorldConfig, WorldData, WorldPosition,
    create_settlement,
};

pub const SETTLEMENT_OVERLAP_FEEDBACK: &str = "Too Close to Existing Settlement";
const REJECTION_FEEDBACK_SECS: f32 = 2.5;

#[derive(Component, Debug, Clone, Copy)]
pub struct DevSettlementPlacementButton;

#[derive(Component, Debug)]
pub struct SettlementPlacementRejectionLabel;

#[derive(Debug, Clone)]
pub struct SettlementPlacementRejection {
    pub position: WorldPosition,
    pub message: String,
    pub expires_at_secs: f32,
}

#[derive(Resource, Default, Debug)]
pub struct SettlementPlacementRejectionFeedbacks {
    pub entries: Vec<SettlementPlacementRejection>,
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct SettlementPlacementPreview {
    pub active: bool,
    pub center: Option<WorldPosition>,
    pub radius_meters: f32,
}

#[derive(Resource, Default, Debug)]
pub struct SettlementPlacementRejectionLabelIndex {
    pub entities: Vec<Entity>,
}

pub fn spawn_settlement_section(_parent: &mut ChildSpawnerCommands<'_>) {
    // Settlement Dev controls live in the Settlement window (settlement_window module).
}

pub fn cancel_settlement_placement(dev_state: &mut DevModeState) {
    dev_state.settlement_placement_armed = false;
    dev_state.settlement_placement_message = "Settlement placement cancelled".into();
}

pub fn handle_settlement_placement_button(
    registry: Res<crate::dev::window::DevWindowRegistry>,
    mut gate: ResMut<DevModeInputGate>,
    mut dev_state: ResMut<DevModeState>,
    buttons: Query<(&Interaction, &DevSettlementPlacementButton), Changed<Interaction>>,
) {
    if !registry.window_active(
        dev_state.enabled,
        crate::dev::window::DevWindowId::Settlement,
    ) {
        return;
    }
    for (interaction, _) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        dev_state.settlement_placement_armed = true;
        dev_state.cancel_placement_tool();
        dev_state.settlement_placement_message =
            "Settlement anchor armed — left-click terrain".into();
    }
}

pub fn sync_settlement_placement_button_active(
    dev_state: Res<DevModeState>,
    mut buttons: Query<
        &mut crate::dev::widgets::DevButtonChrome,
        With<DevSettlementPlacementButton>,
    >,
) {
    let active = dev_state.enabled && dev_state.settlement_placement_armed;
    for mut chrome in &mut buttons {
        chrome.active = active;
    }
}

pub fn update_settlement_placement_preview(
    dev_state: Res<DevModeState>,
    panel_hovered: Res<DevPanelHoverState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    world: Res<WorldData>,
    mut preview: ResMut<SettlementPlacementPreview>,
) {
    preview.active = false;
    preview.center = None;
    if !dev_state.enabled || !dev_state.settlement_placement_armed || panel_hovered.hovered {
        return;
    }

    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let Some(click) = terrain_click_to_world_position(&ray, &world, layout, vertical_scale) else {
        return;
    };
    let Some(position) = dev_spawn_position_from_terrain_click(&world, click.world_position) else {
        return;
    };

    preview.active = true;
    preview.center = Some(position);
    preview.radius_meters = DEFAULT_TOWN_BOUNDARY_RADIUS_METERS;
}

pub fn draw_settlement_placement_preview(
    dev_state: Res<DevModeState>,
    preview: Res<SettlementPlacementPreview>,
    settings: Res<DebugOverlaySettings>,
    config: Res<WorldConfig>,
    world: Res<WorldData>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut gizmos: Gizmos,
) {
    if !dev_state.enabled
        || !dev_state.settlement_placement_armed
        || !preview.active
        || !settings.enabled
        || !settings.category_enabled(DebugOverlayCategory::Interaction)
    {
        return;
    }
    let Some(center) = preview.center else {
        return;
    };
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let center_render = world_position_to_render_global(center, layout, vertical_scale);
    let ring = sample_terrain_ring_render_points(
        center_render,
        preview.radius_meters.max(0.1),
        &world,
        layout,
        vertical_scale,
        SELECTION_RING_SEGMENTS,
    );
    draw_terrain_ring_gizmos(&mut gizmos, &ring, Color::srgba(0.95, 0.82, 0.2, 0.9));
    gizmos.sphere(
        center_render + Vec3::Y * 0.35,
        0.35,
        Color::srgba(0.95, 0.82, 0.2, 0.75),
    );
}

pub fn handle_settlement_placement_click(
    time: Res<Time>,
    mut dev_state: ResMut<DevModeState>,
    mut gate: ResMut<DevModeInputGate>,
    panel_hovered: Res<DevPanelHoverState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut world: ResMut<WorldData>,
    simulation: Res<SimulationControlState>,
    mut feedbacks: ResMut<SettlementPlacementRejectionFeedbacks>,
) {
    if !dev_state.enabled || !dev_state.settlement_placement_armed {
        return;
    }
    if panel_hovered.hovered || gate.spawn_handled_this_frame {
        return;
    }
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(ray) = cursor_world_ray(&windows, &camera) else {
        return;
    };
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let Some(click) = terrain_click_to_world_position(&ray, &world, layout, vertical_scale) else {
        return;
    };
    let Some(position) = dev_spawn_position_from_terrain_click(&world, click.world_position) else {
        return;
    };

    gate.block_gameplay_mouse = true;
    gate.spawn_handled_this_frame = true;

    let ownership = SettlementOwnership {
        owner_id: None,
        team_id: None,
        affiliation: dev_state.spawn_affiliation,
    };
    let tick = simulation.current_tick;
    let message = match place_settlement_anchor(&mut world, position, ownership, tick) {
        Ok(report) => {
            let radius = world
                .settlement_store()
                .get_settlement(report.settlement_id)
                .map(|record| record.boundary_radius_meters)
                .unwrap_or(0.0);
            format!(
                "Created settlement #{} anchor #{} radius {:.0}m — placement still armed",
                report.settlement_id.raw(),
                report.anchor_id.raw(),
                radius
            )
        }
        Err(SettlementCreationError::OverlapsExisting { .. }) => {
            feedbacks.entries.push(SettlementPlacementRejection {
                position,
                message: SETTLEMENT_OVERLAP_FEEDBACK.into(),
                expires_at_secs: time.elapsed_secs() + REJECTION_FEEDBACK_SECS,
            });
            SETTLEMENT_OVERLAP_FEEDBACK.into()
        }
        Err(error) => format!("Settlement placement rejected: {error}"),
    };
    dev_state.settlement_placement_message = message;
}

pub fn sync_settlement_placement_rejection_labels(
    mut commands: Commands,
    time: Res<Time>,
    dev_state: Option<Res<DevModeState>>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut feedbacks: ResMut<SettlementPlacementRejectionFeedbacks>,
    mut label_index: ResMut<SettlementPlacementRejectionLabelIndex>,
    labels: Query<Entity, With<SettlementPlacementRejectionLabel>>,
) {
    let now = time.elapsed_secs();
    feedbacks
        .entries
        .retain(|entry| entry.expires_at_secs > now);

    if dev_state.is_none_or(|state| !state.enabled) {
        for entity in &labels {
            commands.entity(entity).despawn();
        }
        label_index.entities.clear();
        feedbacks.entries.clear();
        return;
    }

    while label_index.entities.len() > feedbacks.entries.len() {
        if let Some(entity) = label_index.entities.pop() {
            commands.entity(entity).despawn();
        }
    }

    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let layout = config.chunk_layout();

    for (index, entry) in feedbacks.entries.iter().enumerate() {
        let translation =
            world_position_to_render_global(entry.position, layout, vertical_scale) + Vec3::Y * 2.5;
        if let Some(entity) = label_index.entities.get(index).copied() {
            commands.entity(entity).insert((
                Transform::from_translation(translation),
                Text2d::new(entry.message.clone()),
                TextColor(Color::srgba(0.98, 0.2, 0.2, 0.98)),
            ));
            continue;
        }
        let entity = commands
            .spawn((
                SettlementPlacementRejectionLabel,
                Text2d::new(entry.message.clone()),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgba(0.98, 0.2, 0.2, 0.98)),
                Transform::from_translation(translation),
                GlobalTransform::default(),
                Visibility::default(),
            ))
            .id();
        label_index.entities.push(entity);
    }
}

pub fn billboard_settlement_placement_rejection_labels(
    camera: Query<&GlobalTransform, With<RtsCamera>>,
    mut labels: Query<&mut Transform, With<SettlementPlacementRejectionLabel>>,
) {
    let Ok(camera_transform) = camera.single() else {
        return;
    };
    let camera_position = camera_transform.translation();
    for mut transform in &mut labels {
        let label_world = transform.translation;
        let to_camera = camera_position - label_world;
        if to_camera.length_squared() < 1e-6 {
            continue;
        }
        let forward = to_camera.normalize();
        let mut right = Vec3::Y.cross(forward);
        if right.length_squared() < 1e-6 {
            right = Vec3::X;
        } else {
            right = right.normalize();
        }
        let up = forward.cross(right);
        transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
    }
}

pub fn clear_settlement_placement_preview_when_disarmed(
    dev_state: Res<DevModeState>,
    mut preview: ResMut<SettlementPlacementPreview>,
) {
    if !dev_state.settlement_placement_armed {
        preview.active = false;
        preview.center = None;
    }
}

pub fn place_settlement_anchor(
    world: &mut WorldData,
    position: WorldPosition,
    ownership: SettlementOwnership,
    tick: u64,
) -> Result<CreateSettlementReport, SettlementCreationError> {
    let building_count_before = world.sorted_building_ids().len();
    let index = world.settlement_store().sorted_settlement_ids().len() + 1;
    let report = create_settlement(
        world,
        position,
        format!("Settlement {index}"),
        ownership,
        SettlementKind::Town,
        None,
        None,
        tick,
    )?;
    if world.sorted_building_ids().len() != building_count_before {
        debug_assert_eq!(
            world.sorted_building_ids().len(),
            building_count_before,
            "canonical dev placement must not create BuildingRecord"
        );
    }
    apply_dev_placement_policy_guard(world, report.settlement_id);
    Ok(report)
}

fn apply_dev_placement_policy_guard(world: &mut WorldData, settlement_id: SettlementId) {
    if let Some(state) = world.settlement_state_store_mut().get_mut(settlement_id) {
        state.policies.automation_enabled = false;
        state.policies.planner_enabled = false;
        state.policies.auto_construction = false;
        state.policies.auto_emergency_response = false;
        state.policies.auto_production_reprioritize = false;
        state.policies.auto_task_interruption = false;
    }
}

pub fn settlement_placement_status(dev_state: &DevModeState) -> String {
    if dev_state.settlement_placement_armed {
        "Settlement anchor armed — left-click terrain".to_string()
    } else if !dev_state.settlement_placement_message.is_empty() {
        dev_state.settlement_placement_message.clone()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::dev_mode::DevModeInputGate;
    use crate::dev::settlement_window::setup_settlement_window_panel;
    use crate::dev::widgets::DevCollapsibleState;
    use crate::dev::window::{DevWindowRegistry, setup_dev_workspace};
    use crate::world::{
        ChunkCoord, ChunkData, ChunkLayout, Heightfield, LocalPosition, SettlementOwnership,
    };
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::MinimalPlugins;

    fn test_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn position(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn dev_placement_creates_no_building_record() {
        let mut world = test_world();
        let before = world.sorted_building_ids().len();
        let report = place_settlement_anchor(
            &mut world,
            position(20.0, 20.0),
            SettlementOwnership::player_default(),
            0,
        )
        .expect("placement");
        assert_eq!(world.sorted_building_ids().len(), before);
        let state = world
            .settlement_state_store()
            .get(report.settlement_id)
            .expect("state");
        assert!(!state.policies.auto_construction);
        assert!(!state.policies.automation_enabled);
    }

    #[test]
    fn armed_state_persists_across_successful_placement_calls() {
        let mut dev = DevModeState::default();
        dev.settlement_placement_armed = true;
        let mut world = test_world();
        place_settlement_anchor(
            &mut world,
            position(30.0, 30.0),
            SettlementOwnership::player_default(),
            0,
        )
        .expect("placement");
        assert!(dev.settlement_placement_armed);
    }

    #[test]
    fn overlap_rejection_creates_no_settlement() {
        let mut world = test_world();
        place_settlement_anchor(
            &mut world,
            position(10.0, 10.0),
            SettlementOwnership::player_default(),
            0,
        )
        .expect("first");
        let count = world.settlement_store().sorted_settlement_ids().len();
        let err = place_settlement_anchor(
            &mut world,
            position(12.0, 12.0),
            SettlementOwnership::player_default(),
            1,
        )
        .expect_err("overlap");
        assert!(matches!(
            err,
            SettlementCreationError::OverlapsExisting { .. }
        ));
        assert_eq!(
            world.settlement_store().sorted_settlement_ids().len(),
            count
        );
    }

    #[test]
    fn placement_succeeds_after_overlap_rejection() {
        let mut world = test_world();
        place_settlement_anchor(
            &mut world,
            position(10.0, 10.0),
            SettlementOwnership::player_default(),
            0,
        )
        .expect("first");
        assert!(
            place_settlement_anchor(
                &mut world,
                position(12.0, 12.0),
                SettlementOwnership::player_default(),
                1,
            )
            .is_err()
        );
        let second = place_settlement_anchor(
            &mut world,
            position(140.0, 140.0),
            SettlementOwnership::player_default(),
            2,
        )
        .expect("second far away");
        assert!(
            world
                .settlement_store()
                .get_settlement(second.settlement_id)
                .is_some()
        );
    }

    #[test]
    fn cancel_clears_armed_state() {
        let mut dev = DevModeState::default();
        dev.settlement_placement_armed = true;
        cancel_settlement_placement(&mut dev);
        assert!(!dev.settlement_placement_armed);
    }

    #[test]
    fn terrain_ring_points_vary_on_sloped_terrain() {
        let mut world = test_world();
        let samples: Vec<f32> = (0..9).map(|i| i as f32).collect();
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        world.insert(
            crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let points = sample_terrain_ring_render_points(
            Vec3::new(64.0, 0.0, 64.0),
            20.0,
            &world,
            ChunkLayout {
                chunk_size_meters: 256.0,
                units_per_meter: 1.0,
            },
            1.0,
            16,
        );
        let min_y = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_y = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
        assert!(max_y - min_y > 0.01);
    }

    #[test]
    fn sync_marks_button_active_when_armed() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::ui::UiPlugin));
        app.init_resource::<DevModeState>()
            .init_resource::<DevWindowRegistry>()
            .init_resource::<DevModeInputGate>()
            .init_resource::<DevCollapsibleState>();
        app.world_mut()
            .run_system_once(setup_dev_workspace)
            .expect("workspace");
        app.world_mut()
            .run_system_once(setup_settlement_window_panel)
            .expect("panel");
        let button = app
            .world_mut()
            .query::<(Entity, &DevSettlementPlacementButton)>()
            .iter(app.world())
            .map(|(entity, _)| entity)
            .next()
            .expect("button");
        {
            let mut dev = app.world_mut().resource_mut::<DevModeState>();
            dev.enabled = true;
            dev.settlement_placement_armed = true;
        }
        app.world_mut()
            .run_system_once(sync_settlement_placement_button_active)
            .expect("sync");
        let chrome = app
            .world()
            .get::<crate::dev::widgets::DevButtonChrome>(button)
            .expect("chrome");
        assert!(chrome.active);
    }
}
