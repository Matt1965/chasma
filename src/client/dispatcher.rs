//! Intent dispatch — routes client intents to selection and command APIs (ADR-038 U-UI2).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::camera::RtsCamera;
use crate::debug::{unit_ids_for_intent, ClientBoundaryGuard, ClientFrameIndex, PendingDispatchTrace, PendingDispatchTraceRecord};
use crate::terrain::TerrainRenderAssets;
use crate::ui::gameplay::MoveCommandFeedback;
use crate::units::input::{
    collect_units_in_screen_rect, issue_attack_move_orders_to_selection,
    issue_attack_orders_to_selection, issue_idle_orders_to_selection, issue_move_orders_to_selection,
    prune_non_commandable_from_selection, MoveOrdersReport, PlayerInteractionSettings, SelectedUnits,
};
use crate::units::UnitRenderEntity;
use crate::world::{
    AttackTargetingPolicy, DoodadCatalog, InteractionOrderPlan, InteractionResolveContext,
    NavigationConfig, NavigationPath, UnitCatalog, UnitId, WeaponCatalog, WorldConfig, WorldData,
    WorldPosition, xz_distance, resolve_unit_click_to_order, resolve_world_click_to_order,
};

use super::commands::{
    build_command_plan, resolve_contextual_command, resolve_palette_command, BuiltCommandPlan,
    CommandResolutionContext, CommandTarget, CommandType,
};
use super::intent::{ClientInputModifiers, ClientIntent, ClientIntentQueue};
use crate::world::{SelectionControllabilityPolicy, unit_is_selectable};

/// Bundled simulation catalogs (keeps dispatch system param count under Bevy limit).
#[derive(SystemParam)]
pub struct DispatchSimulationParams<'w> {
    pub unit_catalog: Res<'w, UnitCatalog>,
    pub weapon_catalog: Res<'w, WeaponCatalog>,
    pub doodad_catalog: Res<'w, DoodadCatalog>,
    pub nav_config: Res<'w, NavigationConfig>,
}

/// Outcome of dispatching one intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentDispatchStatus {
    Applied,
    Ignored,
}

/// Per-intent dispatch record for debug logging.
#[derive(Debug, Clone, PartialEq)]
pub struct IntentDispatchRecord {
    pub intent: ClientIntent,
    pub status: IntentDispatchStatus,
}

/// Aggregated outcome of [`dispatch_client_intents`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IntentDispatchReport {
    pub records: Vec<IntentDispatchRecord>,
}

impl IntentDispatchReport {
    pub fn applied(&self) -> u32 {
        self.records
            .iter()
            .filter(|record| record.status == IntentDispatchStatus::Applied)
            .count() as u32
    }

    pub fn ignored(&self) -> u32 {
        self.records
            .iter()
            .filter(|record| record.status == IntentDispatchStatus::Ignored)
            .count() as u32
    }
}

/// Route queued intents to selection updates and [`issue_unit_order`] dispatch.
pub fn dispatch_client_intents(
    mut queue: ResMut<ClientIntentQueue>,
    mut selection: ResMut<SelectedUnits>,
    mut move_feedback: ResMut<MoveCommandFeedback>,
    mut world: ResMut<WorldData>,
    config: Res<WorldConfig>,
    catalogs: DispatchSimulationParams,
    settings: Res<PlayerInteractionSettings>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    units: Query<(&UnitRenderEntity, &GlobalTransform)>,
    mut modifiers: ResMut<ClientInputModifiers>,
    mut pending_trace: ResMut<PendingDispatchTrace>,
    frame_index: Res<ClientFrameIndex>,
    mut boundary: ResMut<ClientBoundaryGuard>,
) {
    boundary.begin_intent_dispatch();
    pending_trace.clear();
    pending_trace.tick = frame_index.0;
    selection.prune_missing(&world);

    let intents = queue.drain();
    if intents.is_empty() {
        boundary.end_intent_dispatch();
        return;
    }

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    let selection_policy = modifiers.selection_policy;

    let mut report = IntentDispatchReport::default();

    let DispatchSimulationParams {
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
    } = catalogs;

    for intent in intents {
        let move_report_holder;
        let (status, _move_report) = {
            let mut move_report_opt = None;
            let status = dispatch_one(
                &intent,
                &mut selection,
                &mut move_feedback,
                &mut world,
                &unit_catalog,
                &weapon_catalog,
                &doodad_catalog,
                &nav_config,
                layout,
                vertical_scale,
                &settings,
                Some(&camera),
                Some(&units),
                &mut modifiers,
                &mut move_report_opt,
                &mut pending_trace,
                selection_policy,
            );
            move_report_holder = move_report_opt;
            (status, move_report_holder.as_ref())
        };
        if settings.debug_intents {
            log_intent_dispatch(&intent, status);
        }
        let mut affected_units = unit_ids_for_intent(&intent);
        if affected_units.is_empty()
            && matches!(
                intent,
                ClientIntent::MoveCommand { .. } | ClientIntent::ContextualCommand { .. }
            )
        {
            affected_units = selection.iter().collect();
        }
        if matches!(
            intent,
            ClientIntent::BoxSelect { .. } | ClientIntent::BoxSelectAdd { .. }
        ) && status == IntentDispatchStatus::Applied
        {
            affected_units = selection.iter().collect();
        }
        pending_trace.records.push(PendingDispatchTraceRecord {
            intent: intent.clone(),
            status,
            unit_ids: affected_units,
            move_report: move_report_holder,
        });
        report.records.push(IntentDispatchRecord { intent, status });
    }

    pending_trace.report = Some(report);
}

fn dispatch_one(
    intent: &ClientIntent,
    selection: &mut SelectedUnits,
    move_feedback: &mut MoveCommandFeedback,
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
    settings: &PlayerInteractionSettings,
    camera: Option<&Query<(&Camera, &GlobalTransform), With<RtsCamera>>>,
    units: Option<&Query<(&UnitRenderEntity, &GlobalTransform)>>,
    modifiers: &mut ClientInputModifiers,
    move_report: &mut Option<MoveOrdersReport>,
    pending_trace: &mut PendingDispatchTrace,
    selection_policy: SelectionControllabilityPolicy,
) -> IntentDispatchStatus {
    match intent {
        ClientIntent::ContextualCommand { target } => dispatch_contextual_command(
            *target,
            selection,
            move_feedback,
            world,
            unit_catalog,
            weapon_catalog,
            doodad_catalog,
            nav_config,
            layout,
            vertical_scale,
            settings,
            move_report,
            pending_trace,
        ),
        ClientIntent::MoveCommand { target } => dispatch_contextual_command(
            CommandTarget::Terrain { position: *target },
            selection,
            move_feedback,
            world,
            unit_catalog,
            weapon_catalog,
            doodad_catalog,
            nav_config,
            layout,
            vertical_scale,
            settings,
            move_report,
            pending_trace,
        ),
        ClientIntent::SelectUnit { unit_id } => {
            if world
                .get_unit(*unit_id)
                .is_some_and(|record| unit_is_selectable(record, selection_policy))
            {
                selection.set_single(*unit_id);
                IntentDispatchStatus::Applied
            } else {
                IntentDispatchStatus::Ignored
            }
        }
        ClientIntent::ToggleUnitSelection { unit_id } => {
            if world
                .get_unit(*unit_id)
                .is_some_and(|record| unit_is_selectable(record, selection_policy))
            {
                selection.toggle(*unit_id);
                IntentDispatchStatus::Applied
            } else {
                IntentDispatchStatus::Ignored
            }
        }
        ClientIntent::BoxSelect { rect_min, rect_max } => {
            let (camera, units) = match (camera, units) {
                (Some(camera), Some(units)) => (camera, units),
                _ => return IntentDispatchStatus::Ignored,
            };
            let Some(picked) = units_in_screen_rect(
                *rect_min,
                *rect_max,
                camera,
                world,
                units,
                selection_policy,
            ) else {
                return IntentDispatchStatus::Ignored;
            };
            selection.replace_with(picked);
            IntentDispatchStatus::Applied
        }
        ClientIntent::BoxSelectAdd { rect_min, rect_max } => {
            let (camera, units) = match (camera, units) {
                (Some(camera), Some(units)) => (camera, units),
                _ => return IntentDispatchStatus::Ignored,
            };
            let Some(picked) = units_in_screen_rect(
                *rect_min,
                *rect_max,
                camera,
                world,
                units,
                selection_policy,
            ) else {
                return IntentDispatchStatus::Ignored;
            };
            selection.add_all(picked);
            IntentDispatchStatus::Applied
        }
        ClientIntent::ClearSelection => {
            selection.clear();
            IntentDispatchStatus::Applied
        }
        ClientIntent::ShiftModifier { pressed } => {
            modifiers.shift = *pressed;
            IntentDispatchStatus::Applied
        }
        ClientIntent::PaletteCommand { command_type } => dispatch_palette_command(
            *command_type,
            selection,
            move_feedback,
            world,
            unit_catalog,
            weapon_catalog,
            doodad_catalog,
            nav_config,
            layout,
            vertical_scale,
            settings,
            move_report,
            pending_trace,
        ),
    }
}

fn resolve_move_target_from_interaction(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    selected: &[UnitId],
    target: CommandTarget,
) -> Option<WorldPosition> {
    let ctx = InteractionResolveContext::new(
        world,
        doodad_catalog,
        unit_catalog,
        weapon_catalog,
        selected,
    );
    let plan = match target {
        CommandTarget::Terrain { position } => resolve_world_click_to_order(&ctx, position)?,
        CommandTarget::Unit { unit_id } => resolve_unit_click_to_order(&ctx, unit_id)?,
    };
    match plan {
        InteractionOrderPlan::MoveTo { target } => Some(target),
        InteractionOrderPlan::NoOp => None,
        InteractionOrderPlan::Attack { .. } | InteractionOrderPlan::AttackMove { .. } => None,
    }
}

fn dispatch_contextual_command(
    target: CommandTarget,
    selection: &mut SelectedUnits,
    move_feedback: &mut MoveCommandFeedback,
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
    settings: &PlayerInteractionSettings,
    move_report: &mut Option<MoveOrdersReport>,
    pending_trace: &mut PendingDispatchTrace,
) -> IntentDispatchStatus {
    if selection.is_empty() {
        return IntentDispatchStatus::Ignored;
    }

    prune_non_commandable_from_selection(world, selection);
    if selection.is_empty() {
        return IntentDispatchStatus::Ignored;
    }

    let selected: Vec<_> = selection.iter().collect();
    let targeting_policy = AttackTargetingPolicy::default();
    let Some(contextual) = resolve_contextual_command(&CommandResolutionContext {
        selected_units: &selected,
        target,
        world,
        unit_catalog,
        weapon_catalog,
        targeting_policy,
    }) else {
        return IntentDispatchStatus::Ignored;
    };

    let plan = match build_command_plan(&contextual, selection, world) {
        Ok(plan) => plan,
        Err(_) => return IntentDispatchStatus::Ignored,
    };

    pending_trace.resolved_command = Some(contextual.command_type);
    pending_trace.command_tooltip = Some(contextual.command_type.tooltip().to_string());

    match plan {
        BuiltCommandPlan::MoveTo { .. } => {
            let selected_ids: Vec<_> = selection.iter().collect();
            let Some(resolved_target) = resolve_move_target_from_interaction(
                world,
                doodad_catalog,
                unit_catalog,
                weapon_catalog,
                &selected_ids,
                contextual.target,
            ) else {
                return IntentDispatchStatus::Ignored;
            };
            if settings.debug_unit_interaction {
                log_move_target(&resolved_target, layout);
            }
            let move_report_result = issue_move_orders_to_selection(
                world,
                selection,
                unit_catalog,
                weapon_catalog,
                doodad_catalog,
                nav_config,
                resolved_target,
                targeting_policy,
            );
            *move_report = Some(move_report_result);
            move_feedback.set_target(resolved_target, layout, vertical_scale);
            if settings.debug_unit_interaction {
                for unit_id in selected_ids {
                    if let Some(record) = world.get_unit(unit_id) {
                        if let crate::world::UnitState::Moving { ref path, .. } = record.state {
                            log_generated_path(record.placement.position, resolved_target, path, layout);
                        }
                    }
                }
                if let Some(report) = move_report.as_ref() {
                    info!(
                        "multi move issued={} failed={} selected={}",
                        report.issued,
                        report.failed,
                        selection.0.len()
                    );
                }
            }
            IntentDispatchStatus::Applied
        }
        BuiltCommandPlan::Attack { target } => {
            *move_report = Some(issue_attack_orders_to_selection(
                world,
                selection,
                unit_catalog,
                weapon_catalog,
                doodad_catalog,
                nav_config,
                target,
                targeting_policy,
            ));
            IntentDispatchStatus::Applied
        }
        BuiltCommandPlan::AttackMove { destination } => {
            *move_report = Some(issue_attack_move_orders_to_selection(
                world,
                selection,
                unit_catalog,
                weapon_catalog,
                doodad_catalog,
                nav_config,
                destination,
                targeting_policy,
            ));
            IntentDispatchStatus::Applied
        }
        BuiltCommandPlan::StopAll => {
            *move_report = Some(issue_idle_orders_to_selection(
                world,
                unit_catalog,
                weapon_catalog,
                doodad_catalog,
                nav_config,
                selection,
                targeting_policy,
            ));
            IntentDispatchStatus::Applied
        }
        BuiltCommandPlan::HoldAll => {
            *move_report = Some(issue_idle_orders_to_selection(
                world,
                unit_catalog,
                weapon_catalog,
                doodad_catalog,
                nav_config,
                selection,
                targeting_policy,
            ));
            IntentDispatchStatus::Applied
        }
        BuiltCommandPlan::NoOp => IntentDispatchStatus::Ignored,
    }
}

fn dispatch_palette_command(
    command_type: CommandType,
    selection: &mut SelectedUnits,
    _move_feedback: &mut MoveCommandFeedback,
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    _layout: crate::world::ChunkLayout,
    _vertical_scale: f32,
    _settings: &PlayerInteractionSettings,
    move_report: &mut Option<MoveOrdersReport>,
    pending_trace: &mut PendingDispatchTrace,
) -> IntentDispatchStatus {
    if selection.is_empty() {
        return IntentDispatchStatus::Ignored;
    }

    prune_non_commandable_from_selection(world, selection);
    if selection.is_empty() {
        return IntentDispatchStatus::Ignored;
    }

    let selected: Vec<_> = selection.iter().collect();
    let targeting_policy = AttackTargetingPolicy::default();
    let Some(contextual) = resolve_palette_command(command_type, &selected, None) else {
        return IntentDispatchStatus::Ignored;
    };

    let plan = match build_command_plan(&contextual, selection, world) {
        Ok(plan) => plan,
        Err(_) => return IntentDispatchStatus::Ignored,
    };

    pending_trace.resolved_command = Some(contextual.command_type);
    pending_trace.command_tooltip = Some(contextual.command_type.tooltip().to_string());

    match plan {
        BuiltCommandPlan::MoveTo { .. } => IntentDispatchStatus::Ignored,
        BuiltCommandPlan::AttackMove { destination } => {
            *move_report = Some(issue_attack_move_orders_to_selection(
                world,
                selection,
                unit_catalog,
                weapon_catalog,
                doodad_catalog,
                nav_config,
                destination,
                targeting_policy,
            ));
            IntentDispatchStatus::Applied
        }
        BuiltCommandPlan::Attack { .. } => IntentDispatchStatus::Ignored,
        BuiltCommandPlan::StopAll => {
            *move_report = Some(issue_idle_orders_to_selection(
                world,
                unit_catalog,
                weapon_catalog,
                doodad_catalog,
                nav_config,
                selection,
                targeting_policy,
            ));
            IntentDispatchStatus::Applied
        }
        BuiltCommandPlan::HoldAll => {
            *move_report = Some(issue_idle_orders_to_selection(
                world,
                unit_catalog,
                weapon_catalog,
                doodad_catalog,
                nav_config,
                selection,
                targeting_policy,
            ));
            IntentDispatchStatus::Applied
        }
        BuiltCommandPlan::NoOp => IntentDispatchStatus::Ignored,
    }
}

fn units_in_screen_rect(
    rect_min: Vec2,
    rect_max: Vec2,
    camera: &Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    world: &WorldData,
    units: &Query<(&UnitRenderEntity, &GlobalTransform)>,
    policy: SelectionControllabilityPolicy,
) -> Option<std::collections::HashSet<crate::world::UnitId>> {
    let (camera, camera_transform) = camera.single().ok()?;
    Some(collect_units_in_screen_rect(
        rect_min,
        rect_max,
        camera,
        camera_transform,
        world,
        units,
        policy,
    ))
}

fn log_intent_dispatch(intent: &ClientIntent, status: IntentDispatchStatus) {
    info!("intent dispatch {intent:?} -> {status:?}");
}

fn log_move_target(target: &WorldPosition, layout: crate::world::ChunkLayout) {
    let global = target.to_global(layout);
    info!(
        "move intent target chunk=({}, {}) local=({:.2}, {:.2}) global=({:.2}, {:.2}, {:.2})",
        target.chunk.x,
        target.chunk.z,
        target.local.0.x,
        target.local.0.z,
        global.x,
        global.y,
        global.z,
    );
}

fn log_generated_path(
    start: WorldPosition,
    goal: WorldPosition,
    path: &NavigationPath,
    layout: crate::world::ChunkLayout,
) {
    let straight = xz_distance(start, goal, layout);
    let path_len = path.length_meters(layout);
    let ratio = if straight > 1e-4 {
        path_len / straight
    } else {
        1.0
    };
    info!(
        "path start=({:.2}, {:.2}) goal=({:.2}, {:.2}) waypoints={} length={:.2} straight={:.2} ratio={:.3}",
        start.to_global(layout).x,
        start.to_global(layout).z,
        goal.to_global(layout).x,
        goal.to_global(layout).z,
        path.len(),
        path_len,
        straight,
        ratio,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::commands::CommandType;
    use crate::units::input::SelectedUnits;
    use crate::world::{
        create_doodad, create_unit, create_unit_with_ownership, resolve_all_pending_unit_orders,
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadDefinitionId, DoodadPlacementOverrides,
        DoodadSource, Heightfield, LocalPosition, UnitDefinitionId, UnitOwnership, UnitSource,
        UnitState, WorldPosition,
    };
    use bevy::prelude::{Vec2, Vec3};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(layout());
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

    #[test]
    fn dispatcher_routes_select_unit_intent() {
        let mut selection = SelectedUnits::default();
        let mut move_feedback = MoveCommandFeedback::default();
        let mut world = flat_world();
        let mut modifiers = ClientInputModifiers::default();
        let catalog = UnitCatalog::default();
        let unit_id = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(4.0, 4.0),
            UnitSource::Authored,
            crate::world::UnitOwnership::player_default(),
        )
        .unwrap()
        .id;

        let status = dispatch_one(
            &ClientIntent::SelectUnit { unit_id },
            &mut selection,
            &mut move_feedback,
            &mut world,
            &catalog,
            &WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            layout(),
            1.0,
            &PlayerInteractionSettings::default(),
            None,
            None,
            &mut modifiers,
            &mut None,
            &mut PendingDispatchTrace::default(),
            SelectionControllabilityPolicy::gameplay_default(),
        );
        assert_eq!(status, IntentDispatchStatus::Applied);
        assert!(selection.contains(unit_id));
    }

    #[test]
    fn dispatcher_routes_move_command_intent() {
        let mut selection = SelectedUnits::default();
        let mut move_feedback = MoveCommandFeedback::default();
        let mut world = flat_world();
        let mut modifiers = ClientInputModifiers::default();
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let unit_id = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(4.0, 4.0),
            UnitSource::Authored,
            crate::world::UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        selection.set_single(unit_id);

        let target = pos(40.0, 40.0);
        let status = dispatch_one(
            &ClientIntent::MoveCommand { target },
            &mut selection,
            &mut move_feedback,
            &mut world,
            &catalog,
            &WeaponCatalog::default(),
            &doodad_catalog,
            &nav_config,
            layout(),
            1.0,
            &PlayerInteractionSettings::default(),
            None,
            None,
            &mut modifiers,
            &mut None,
            &mut PendingDispatchTrace::default(),
            SelectionControllabilityPolicy::gameplay_default(),
        );
        assert_eq!(status, IntentDispatchStatus::Applied);
        resolve_all_pending_unit_orders(&mut world, &catalog, &doodad_catalog, &nav_config);
        assert!(matches!(
            world.get_unit(unit_id).unwrap().state,
            UnitState::Moving { .. }
        ));
    }

    #[test]
    fn move_command_ignored_when_selection_empty() {
        let mut selection = SelectedUnits::default();
        let mut move_feedback = MoveCommandFeedback::default();
        let mut world = flat_world();
        let mut modifiers = ClientInputModifiers::default();
        let catalog = UnitCatalog::default();

        let status = dispatch_one(
            &ClientIntent::MoveCommand {
                target: pos(10.0, 10.0),
            },
            &mut selection,
            &mut move_feedback,
            &mut world,
            &catalog,
            &WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            layout(),
            1.0,
            &PlayerInteractionSettings::default(),
            None,
            None,
            &mut modifiers,
            &mut None,
            &mut PendingDispatchTrace::default(),
            SelectionControllabilityPolicy::gameplay_default(),
        );
        assert_eq!(status, IntentDispatchStatus::Ignored);
    }

    #[test]
    fn move_command_ignored_on_blocked_tree() {
        let mut selection = SelectedUnits::default();
        let mut move_feedback = MoveCommandFeedback::default();
        let mut world = flat_world();
        let mut modifiers = ClientInputModifiers::default();
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let unit_id = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(4.0, 4.0),
            UnitSource::Authored,
            crate::world::UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        selection.set_single(unit_id);
        let tree_pos = pos(50.0, 50.0);
        create_doodad(
            &doodad_catalog,
            &mut world,
            &DoodadDefinitionId::new("tree_oak"),
            tree_pos,
            DoodadSource::Authored,
            DoodadPlacementOverrides::default(),
        )
        .unwrap();
        let state_before = world.get_unit(unit_id).unwrap().state.clone();

        let status = dispatch_one(
            &ClientIntent::MoveCommand { target: tree_pos },
            &mut selection,
            &mut move_feedback,
            &mut world,
            &catalog,
            &WeaponCatalog::default(),
            &doodad_catalog,
            &nav_config,
            layout(),
            1.0,
            &PlayerInteractionSettings::default(),
            None,
            None,
            &mut modifiers,
            &mut None,
            &mut PendingDispatchTrace::default(),
            SelectionControllabilityPolicy::gameplay_default(),
        );
        assert_eq!(status, IntentDispatchStatus::Ignored);
        assert_eq!(world.get_unit(unit_id).unwrap().state, state_before);
        assert!(!move_feedback.has_active_marker());
    }

    #[test]
    fn box_select_ignored_without_render_queries() {
        let mut selection = SelectedUnits::default();
        let mut move_feedback = MoveCommandFeedback::default();
        let mut world = flat_world();
        let mut modifiers = ClientInputModifiers::default();
        let catalog = UnitCatalog::default();

        let status = dispatch_one(
            &ClientIntent::BoxSelect {
                rect_min: Vec2::ZERO,
                rect_max: Vec2::ONE,
            },
            &mut selection,
            &mut move_feedback,
            &mut world,
            &catalog,
            &WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            layout(),
            1.0,
            &PlayerInteractionSettings::default(),
            None,
            None,
            &mut modifiers,
            &mut None,
            &mut PendingDispatchTrace::default(),
            SelectionControllabilityPolicy::gameplay_default(),
        );
        assert_eq!(status, IntentDispatchStatus::Ignored);
    }

    #[test]
    fn select_intent_does_not_mutate_world_units() {
        let mut selection = SelectedUnits::default();
        let mut move_feedback = MoveCommandFeedback::default();
        let mut world = flat_world();
        let mut modifiers = ClientInputModifiers::default();
        let catalog = UnitCatalog::default();
        let unit_id = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(4.0, 4.0),
            UnitSource::Authored,
            crate::world::UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        let state_before = world.get_unit(unit_id).unwrap().state.clone();

        dispatch_one(
            &ClientIntent::SelectUnit { unit_id },
            &mut selection,
            &mut move_feedback,
            &mut world,
            &catalog,
            &WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            layout(),
            1.0,
            &PlayerInteractionSettings::default(),
            None,
            None,
            &mut modifiers,
            &mut None,
            &mut PendingDispatchTrace::default(),
            SelectionControllabilityPolicy::gameplay_default(),
        );

        assert_eq!(world.get_unit(unit_id).unwrap().state, state_before);
    }

    #[test]
    fn contextual_command_routes_through_command_builder() {
        let mut selection = SelectedUnits::default();
        let mut move_feedback = MoveCommandFeedback::default();
        let mut world = flat_world();
        let mut modifiers = ClientInputModifiers::default();
        let mut pending = PendingDispatchTrace::default();
        let catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let nav_config = NavigationConfig::default();
        let unit_id = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(4.0, 4.0),
            UnitSource::Authored,
            crate::world::UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        selection.set_single(unit_id);

        let target = pos(40.0, 40.0);
        let status = dispatch_one(
            &ClientIntent::ContextualCommand {
                target: CommandTarget::Terrain { position: target },
            },
            &mut selection,
            &mut move_feedback,
            &mut world,
            &catalog,
            &WeaponCatalog::default(),
            &doodad_catalog,
            &nav_config,
            layout(),
            1.0,
            &PlayerInteractionSettings::default(),
            None,
            None,
            &mut modifiers,
            &mut None,
            &mut pending,
            SelectionControllabilityPolicy::gameplay_default(),
        );
        assert_eq!(status, IntentDispatchStatus::Applied);
        assert_eq!(pending.resolved_command, Some(CommandType::Move));
        resolve_all_pending_unit_orders(&mut world, &catalog, &doodad_catalog, &nav_config);
        assert!(matches!(
            world.get_unit(unit_id).unwrap().state,
            UnitState::Moving { .. }
        ));
    }

    #[test]
    fn shift_modifier_intent_updates_modifiers() {
        let mut selection = SelectedUnits::default();
        let mut move_feedback = MoveCommandFeedback::default();
        let mut world = flat_world();
        let mut modifiers = ClientInputModifiers::default();
        let catalog = UnitCatalog::default();

        dispatch_one(
            &ClientIntent::ShiftModifier { pressed: true },
            &mut selection,
            &mut move_feedback,
            &mut world,
            &catalog,
            &WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            layout(),
            1.0,
            &PlayerInteractionSettings::default(),
            None,
            None,
            &mut modifiers,
            &mut None,
            &mut PendingDispatchTrace::default(),
            SelectionControllabilityPolicy::gameplay_default(),
        );
        assert!(modifiers.shift);
    }
}
