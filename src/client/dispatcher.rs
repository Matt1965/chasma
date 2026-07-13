//! Intent dispatch — routes client intents to selection and command APIs (ADR-038 U-UI2).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::camera::RtsCamera;
use crate::debug::{
    ClientBoundaryGuard, ClientFrameIndex, PendingDispatchTrace, PendingDispatchTraceRecord,
    unit_ids_for_intent,
};
use crate::terrain::TerrainRenderAssets;
use crate::ui::gameplay::MoveCommandFeedback;
use crate::units::UnitRenderEntity;
use crate::units::input::{
    MoveOrdersReport, PlayerInteractionSettings, SelectedUnits, collect_units_in_screen_rect,
    issue_attack_move_orders_to_selection, issue_attack_orders_to_selection,
    issue_idle_orders_to_selection, issue_move_orders_to_selection,
    prune_non_commandable_from_selection,
};
use crate::world::{
    AttackTargetingPolicy, BuildingCatalog, DoodadCatalog, FootprintCatalog, InteractionOrderPlan,
    InteractionResolveContext, NavigationConfig, NavigationPath, PassabilityCatalogs, UnitCatalog,
    UnitId, WeaponCatalog, WorldConfig, WorldData, WorldPosition, assign_construct_building_task,
    assign_operate_workstation_task, filter_commandable_unit_ids, resolve_unit_click_to_order,
    resolve_world_click_to_order, xz_distance,
};

use super::commands::{
    BuiltCommandPlan, CommandBuildError, CommandResolutionContext, CommandTarget, CommandType,
    CommandUnavailableReason, build_command_plan, command_availability, command_tooltip,
    resolve_contextual_command_with_armed, resolve_palette_command,
};
use super::intent::{ClientInputModifiers, ClientIntent, ClientIntentQueue};
use crate::ui::gameplay::PlayerHudState;
use crate::ui::gameplay::build_mode::BuildModeState;
use crate::world::{
    BuildingOwnership, BuildingPlacementConfig, BuildingPlacementContext, OccupancyCatalogs,
    SelectionControllabilityPolicy, place_player_building, unit_is_selectable,
    validate_building_placement,
};

/// Bundled player/build-mode params for intent dispatch.
#[derive(SystemParam)]
pub struct DispatchPlayerParams<'w> {
    pub build_mode: ResMut<'w, BuildModeState>,
    pub player_ownership: Res<'w, crate::player::LocalPlayerOwnership>,
}

/// Bundled simulation catalogs (keeps dispatch system param count under Bevy limit).
#[derive(SystemParam)]
pub struct DispatchSimulationParams<'w> {
    pub unit_catalog: Res<'w, UnitCatalog>,
    pub weapon_catalog: Res<'w, WeaponCatalog>,
    pub doodad_catalog: Res<'w, DoodadCatalog>,
    pub building_catalog: Res<'w, BuildingCatalog>,
    pub footprint_catalog: Res<'w, FootprintCatalog>,
    pub interaction_catalog: Res<'w, crate::world::BuildingInteractionProfileCatalog>,
    pub nav_config: Res<'w, NavigationConfig>,
}

/// Outcome of dispatching one intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentDispatchStatus {
    Applied,
    Ignored,
    Rejected(CommandUnavailableReason),
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

    /// Intents that attempted a command/selection change but failed validation or availability.
    pub fn rejected(&self) -> u32 {
        self.records
            .iter()
            .filter(|record| matches!(record.status, IntentDispatchStatus::Rejected(_)))
            .count() as u32
    }

    pub fn total(&self) -> u32 {
        self.records.len() as u32
    }

    /// Group rejected intents by unavailable reason (deterministic first-seen order).
    pub fn rejected_reason_counts(&self) -> Vec<(CommandUnavailableReason, u32)> {
        use std::collections::HashMap;
        let mut counts: HashMap<CommandUnavailableReason, u32> = HashMap::new();
        for record in &self.records {
            if let IntentDispatchStatus::Rejected(reason) = record.status {
                *counts.entry(reason).or_insert(0) += 1;
            }
        }
        let mut rows: Vec<_> = counts.into_iter().collect();
        rows.sort_by_key(|(reason, _)| format!("{reason:?}"));
        rows
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
    mut hud: ResMut<PlayerHudState>,
    mut player_params: DispatchPlayerParams,
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
        building_catalog,
        footprint_catalog,
        interaction_catalog,
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
                &building_catalog,
                &footprint_catalog,
                &interaction_catalog,
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
                hud.armed_command,
                &mut player_params.build_mode,
                &player_params.player_ownership,
                frame_index.0,
            );
            if status == IntentDispatchStatus::Applied
                && matches!(
                    intent,
                    ClientIntent::ContextualCommand { .. } | ClientIntent::MoveCommand { .. }
                )
                && hud.armed_command.is_some()
            {
                hud.armed_command = None;
            }
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
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interaction_catalog: &crate::world::BuildingInteractionProfileCatalog,
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
    armed_command: Option<CommandType>,
    build_mode: &mut BuildModeState,
    player_ownership: &crate::player::LocalPlayerOwnership,
    simulation_tick: u64,
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
            building_catalog,
            footprint_catalog,
            interaction_catalog,
            nav_config,
            layout,
            vertical_scale,
            settings,
            move_report,
            pending_trace,
            armed_command,
            simulation_tick,
        ),
        ClientIntent::MoveCommand { target } => dispatch_contextual_command(
            CommandTarget::Terrain { position: *target },
            selection,
            move_feedback,
            world,
            unit_catalog,
            weapon_catalog,
            doodad_catalog,
            building_catalog,
            footprint_catalog,
            interaction_catalog,
            nav_config,
            layout,
            vertical_scale,
            settings,
            move_report,
            pending_trace,
            armed_command,
            simulation_tick,
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
            let Some(picked) =
                units_in_screen_rect(*rect_min, *rect_max, camera, world, units, selection_policy)
            else {
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
            let Some(picked) =
                units_in_screen_rect(*rect_min, *rect_max, camera, world, units, selection_policy)
            else {
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
        ClientIntent::EnterBuildMode => {
            build_mode.enter_catalog();
            IntentDispatchStatus::Applied
        }
        ClientIntent::ExitBuildMode => {
            build_mode.exit();
            IntentDispatchStatus::Applied
        }
        ClientIntent::CancelBuildPlacement => {
            if build_mode.is_ghost_placing() {
                build_mode.cancel_ghost();
            } else if build_mode.is_active() {
                build_mode.exit();
            } else {
                return IntentDispatchStatus::Ignored;
            }
            IntentDispatchStatus::Applied
        }
        ClientIntent::RotateBuildGhost => {
            if build_mode.is_ghost_placing() {
                build_mode.rotate_ghost();
                IntentDispatchStatus::Applied
            } else {
                IntentDispatchStatus::Ignored
            }
        }
        ClientIntent::SelectBuildingDefinition { definition_id } => {
            if building_catalog
                .get(definition_id)
                .is_some_and(|def| def.enabled)
            {
                build_mode.arm_definition(definition_id.clone());
                IntentDispatchStatus::Applied
            } else {
                IntentDispatchStatus::Rejected(CommandUnavailableReason::InvalidPlacement)
            }
        }
        ClientIntent::PlaceBuilding {
            definition_id,
            anchor,
            rotation,
        } => dispatch_place_building(
            definition_id,
            *anchor,
            *rotation,
            world,
            unit_catalog,
            building_catalog,
            footprint_catalog,
            doodad_catalog,
            player_ownership,
            build_mode,
        ),
    }
}

fn dispatch_place_building(
    definition_id: &crate::world::BuildingDefinitionId,
    anchor: WorldPosition,
    rotation: Quat,
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    doodad_catalog: &DoodadCatalog,
    player_ownership: &crate::player::LocalPlayerOwnership,
    build_mode: &mut BuildModeState,
) -> IntentDispatchStatus {
    let ownership = BuildingOwnership {
        owner_id: Some(player_ownership.owner_id),
        team_id: Some(player_ownership.team_id),
        affiliation: crate::world::Affiliation::Player,
    };
    let ctx = BuildingPlacementContext {
        world,
        building_catalog,
        footprint_catalog,
        doodad_catalog,
        unit_catalog,
        config: BuildingPlacementConfig::default(),
        player_authorized: true,
    };
    let validation = validate_building_placement(&ctx, definition_id, anchor, rotation, ownership);
    if !validation.valid {
        build_mode.last_validation = Some(validation);
        return IntentDispatchStatus::Rejected(CommandUnavailableReason::InvalidPlacement);
    }
    let Some(grounded) = validation.grounded_anchor else {
        return IntentDispatchStatus::Rejected(CommandUnavailableReason::InvalidPlacement);
    };
    let occupancy = OccupancyCatalogs {
        doodad: doodad_catalog,
        building: building_catalog,
        footprint: footprint_catalog,
    };
    match place_player_building(
        building_catalog,
        world,
        definition_id,
        grounded,
        rotation,
        ownership,
        occupancy,
    ) {
        Ok(_) => {
            build_mode.cancel_ghost();
            IntentDispatchStatus::Applied
        }
        Err(_) => IntentDispatchStatus::Rejected(CommandUnavailableReason::InvalidPlacement),
    }
}

fn resolve_move_target_from_interaction(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interaction_catalog: &crate::world::BuildingInteractionProfileCatalog,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    selected: &[UnitId],
    target: CommandTarget,
) -> Option<WorldPosition> {
    let ctx = InteractionResolveContext::new(
        world,
        doodad_catalog,
        building_catalog,
        footprint_catalog,
        interaction_catalog,
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
        InteractionOrderPlan::ConstructBuilding { .. }
        | InteractionOrderPlan::OperateWorkstation { .. } => None,
    }
}

fn try_issue_building_work_orders(
    world: &mut WorldData,
    selection: &SelectedUnits,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interaction_catalog: &crate::world::BuildingInteractionProfileCatalog,
    nav_config: &NavigationConfig,
    position: WorldPosition,
    simulation_tick: u64,
) -> Option<MoveOrdersReport> {
    let selected: Vec<_> = selection.iter().collect();
    let ctx = InteractionResolveContext::new(
        world,
        doodad_catalog,
        building_catalog,
        footprint_catalog,
        interaction_catalog,
        unit_catalog,
        weapon_catalog,
        &selected,
    );
    let plan = resolve_world_click_to_order(&ctx, position)?;
    let (building_id, construct) = match plan {
        InteractionOrderPlan::ConstructBuilding { building_id } => (building_id, true),
        InteractionOrderPlan::OperateWorkstation { building_id } => (building_id, false),
        _ => return None,
    };

    let mut report = MoveOrdersReport::default();
    for unit_id in filter_commandable_unit_ids(world, selection.iter()) {
        let result = if construct {
            assign_construct_building_task(
                world,
                unit_catalog,
                weapon_catalog,
                doodad_catalog,
                building_catalog,
                interaction_catalog,
                nav_config,
                unit_id,
                building_id,
                simulation_tick,
            )
        } else {
            assign_operate_workstation_task(
                world,
                unit_catalog,
                weapon_catalog,
                doodad_catalog,
                building_catalog,
                interaction_catalog,
                nav_config,
                unit_id,
                building_id,
                simulation_tick,
            )
        };
        match result {
            Ok((_, _)) => {
                report.issued += 1;
            }
            Err(_) => report.failed += 1,
        }
    }
    if report.issued > 0 {
        Some(report)
    } else {
        None
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
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interaction_catalog: &crate::world::BuildingInteractionProfileCatalog,
    nav_config: &NavigationConfig,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
    settings: &PlayerInteractionSettings,
    move_report: &mut Option<MoveOrdersReport>,
    pending_trace: &mut PendingDispatchTrace,
    armed_command: Option<CommandType>,
    simulation_tick: u64,
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
    let Some(contextual) = resolve_contextual_command_with_armed(
        &CommandResolutionContext {
            selected_units: &selected,
            target,
            world,
            unit_catalog,
            weapon_catalog,
            targeting_policy,
        },
        armed_command,
    ) else {
        return IntentDispatchStatus::Ignored;
    };

    let plan = match build_command_plan(&contextual, selection, world) {
        Ok(plan) => plan,
        Err(CommandBuildError::FeatureUnavailable(reason)) => {
            pending_trace.resolved_command = Some(contextual.command_type);
            pending_trace.command_tooltip = Some(command_tooltip(
                contextual.command_type,
                crate::client::commands::CommandAvailability::Unavailable(reason),
            ));
            pending_trace.unavailable_reason = Some(reason);
            return IntentDispatchStatus::Rejected(reason);
        }
        Err(_) => return IntentDispatchStatus::Ignored,
    };

    pending_trace.resolved_command = Some(contextual.command_type);
    pending_trace.command_tooltip = Some(command_tooltip(
        contextual.command_type,
        command_availability(contextual.command_type, selection),
    ));

    match plan {
        BuiltCommandPlan::MoveTo { .. } => {
            if let CommandTarget::Terrain { position } = contextual.target {
                if let Some(work_report) = try_issue_building_work_orders(
                    world,
                    selection,
                    unit_catalog,
                    weapon_catalog,
                    doodad_catalog,
                    building_catalog,
                    footprint_catalog,
                    interaction_catalog,
                    nav_config,
                    position,
                    simulation_tick,
                ) {
                    *move_report = Some(work_report);
                    return IntentDispatchStatus::Applied;
                }
            }
            let selected_ids: Vec<_> = selection.iter().collect();
            let Some(resolved_target) = resolve_move_target_from_interaction(
                world,
                doodad_catalog,
                building_catalog,
                footprint_catalog,
                interaction_catalog,
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
                            log_generated_path(
                                record.placement.position,
                                resolved_target,
                                path,
                                layout,
                            );
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

    let availability = command_availability(command_type, selection);
    if let Some(reason) = availability.reason() {
        pending_trace.resolved_command = Some(command_type);
        pending_trace.command_tooltip = Some(command_tooltip(command_type, availability));
        pending_trace.unavailable_reason = Some(reason);
        return IntentDispatchStatus::Rejected(reason);
    }

    let selected: Vec<_> = selection.iter().collect();
    let targeting_policy = AttackTargetingPolicy::default();
    let Some(contextual) = resolve_palette_command(command_type, &selected, None) else {
        return IntentDispatchStatus::Ignored;
    };

    let plan = match build_command_plan(&contextual, selection, world) {
        Ok(plan) => plan,
        Err(CommandBuildError::FeatureUnavailable(reason)) => {
            pending_trace.resolved_command = Some(command_type);
            pending_trace.command_tooltip = Some(command_tooltip(command_type, availability));
            pending_trace.unavailable_reason = Some(reason);
            return IntentDispatchStatus::Rejected(reason);
        }
        Err(_) => return IntentDispatchStatus::Ignored,
    };

    pending_trace.resolved_command = Some(contextual.command_type);
    pending_trace.command_tooltip = Some(command_tooltip(
        contextual.command_type,
        command_availability(contextual.command_type, selection),
    ));

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
    use crate::player::LocalPlayerOwnership;
    use crate::ui::gameplay::BuildModeState;
    use crate::units::input::SelectedUnits;
    use crate::world::{
        BuildingCatalog, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog,
        DoodadDefinitionId, DoodadPlacementOverrides, DoodadSource, FootprintCatalog, Heightfield,
        LocalPosition, PassabilityCatalogs, UnitDefinitionId, UnitOwnership, UnitSource, UnitState,
        WorldPosition, create_doodad, create_unit, create_unit_with_ownership,
        resolve_all_pending_unit_orders,
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
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
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
            None,
            &mut BuildModeState::default(),
            &LocalPlayerOwnership::default(),
            0,
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
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
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
            None,
            &mut BuildModeState::default(),
            &LocalPlayerOwnership::default(),
            0,
        );
        assert_eq!(status, IntentDispatchStatus::Applied);
        resolve_all_pending_unit_orders(
            &mut world,
            &catalog,
            PassabilityCatalogs {
                doodad: &doodad_catalog,
                building: &BuildingCatalog::default(),
                footprint: &FootprintCatalog::default(),
            },
            &nav_config,
        );
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
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
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
            None,
            &mut BuildModeState::default(),
            &LocalPlayerOwnership::default(),
            0,
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
            None,
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
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
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
            None,
            &mut BuildModeState::default(),
            &LocalPlayerOwnership::default(),
            0,
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
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
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
            None,
            &mut BuildModeState::default(),
            &LocalPlayerOwnership::default(),
            0,
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
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
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
            None,
            &mut BuildModeState::default(),
            &LocalPlayerOwnership::default(),
            0,
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
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
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
            None,
            &mut BuildModeState::default(),
            &LocalPlayerOwnership::default(),
            0,
        );
        assert_eq!(status, IntentDispatchStatus::Applied);
        assert_eq!(pending.resolved_command, Some(CommandType::Move));
        resolve_all_pending_unit_orders(
            &mut world,
            &catalog,
            PassabilityCatalogs {
                doodad: &doodad_catalog,
                building: &BuildingCatalog::default(),
                footprint: &FootprintCatalog::default(),
            },
            &nav_config,
        );
        assert!(matches!(
            world.get_unit(unit_id).unwrap().state,
            UnitState::Moving { .. }
        ));
    }

    #[test]
    fn palette_hold_position_rejected_without_world_mutation() {
        use crate::client::commands::CommandUnavailableReason;

        let mut selection = SelectedUnits::default();
        let mut move_feedback = MoveCommandFeedback::default();
        let mut world = flat_world();
        let mut modifiers = ClientInputModifiers::default();
        let mut pending = PendingDispatchTrace::default();
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
        selection.set_single(unit_id);
        let state_before = world.get_unit(unit_id).unwrap().state.clone();

        let status = dispatch_one(
            &ClientIntent::PaletteCommand {
                command_type: CommandType::HoldPosition,
            },
            &mut selection,
            &mut move_feedback,
            &mut world,
            &catalog,
            &WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
            &NavigationConfig::default(),
            layout(),
            1.0,
            &PlayerInteractionSettings::default(),
            None,
            None,
            &mut modifiers,
            &mut None,
            &mut pending,
            SelectionControllabilityPolicy::gameplay_default(),
            None,
            &mut BuildModeState::default(),
            &LocalPlayerOwnership::default(),
            0,
        );
        assert_eq!(
            status,
            IntentDispatchStatus::Rejected(CommandUnavailableReason::FeatureNotImplemented)
        );
        assert_eq!(world.get_unit(unit_id).unwrap().state, state_before);
        assert_eq!(
            pending.unavailable_reason,
            Some(CommandUnavailableReason::FeatureNotImplemented)
        );
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
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::BuildingInteractionProfileCatalog::default(),
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
            None,
            &mut BuildModeState::default(),
            &LocalPlayerOwnership::default(),
            0,
        );
        assert!(modifiers.shift);
    }

    #[test]
    fn dispatch_report_partitions_terminal_categories() {
        let report = IntentDispatchReport {
            records: vec![
                IntentDispatchRecord {
                    intent: ClientIntent::ShiftModifier { pressed: true },
                    status: IntentDispatchStatus::Applied,
                },
                IntentDispatchRecord {
                    intent: ClientIntent::ClearSelection,
                    status: IntentDispatchStatus::Ignored,
                },
                IntentDispatchRecord {
                    intent: ClientIntent::PaletteCommand {
                        command_type: CommandType::HoldPosition,
                    },
                    status: IntentDispatchStatus::Rejected(
                        CommandUnavailableReason::FeatureNotImplemented,
                    ),
                },
            ],
        };
        assert_eq!(report.total(), 3);
        assert_eq!(report.applied(), 1);
        assert_eq!(report.ignored(), 1);
        assert_eq!(report.rejected(), 1);
        assert_eq!(
            report.applied() + report.ignored() + report.rejected(),
            report.total()
        );
        assert_eq!(
            report.rejected_reason_counts(),
            vec![(CommandUnavailableReason::FeatureNotImplemented, 1)]
        );
    }
}
