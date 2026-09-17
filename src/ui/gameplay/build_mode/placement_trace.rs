//! DEV-only runtime trace for player build-mode placement chain.

use bevy::prelude::*;

use crate::terrain::{terrain_surface_render_y_at, terrain_surface_sim_y_at};
use crate::world::{
    BuildingAuthoringError, BuildingRecord, BuildingDefinition, BuildingPlacementPlan,
    BuildingPlacementRejectReason, BuildingPlacementValidation, ChunkLayout, FootprintCatalog,
    FOUNDATION_SLOPE_DEGREES, QuantizedRotation, TerrainPlacementMode, WorldData, WorldPosition,
    building_placement_render_y, effective_building_footprint_for_placement,
    footprint_horizontal_span_meters, presentation_foundation_depth_meters,
    presentation_plane_normal, sample_terrain_under_footprint,
    slope_degrees_from_plane_coefficients,
};

/// Toggle with `KeyT` while ghost-placing (DEV builds only).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct BuildModePlacementTrace {
    pub enabled: bool,
    last_signature: Option<u64>,
}

impl BuildModePlacementTrace {
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if self.enabled {
            info!("build placement trace: ON (KeyT toggles)");
        } else {
            info!("build placement trace: OFF");
            self.last_signature = None;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementTracePhase {
    Preview,
    Commit,
}

pub fn handle_placement_trace_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    build_mode: Res<super::state::BuildModeState>,
    mut trace: ResMut<BuildModePlacementTrace>,
) {
    if !build_mode.is_ghost_placing() {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        trace.toggle();
    }
}

pub fn maybe_trace_build_mode_preview(
    trace: &mut BuildModePlacementTrace,
    world: &WorldData,
    footprint_catalog: &FootprintCatalog,
    definition: &BuildingDefinition,
    candidate_anchor: WorldPosition,
    yaw_degrees: f32,
    layout: ChunkLayout,
    vertical_scale: f32,
    validation: &BuildingPlacementValidation,
    plan: Option<&BuildingPlacementPlan>,
) {
    if !trace.enabled {
        return;
    }
    let signature = preview_signature(layout, candidate_anchor, yaw_degrees, validation, plan);
    if trace.last_signature == Some(signature) {
        return;
    }
    trace.last_signature = Some(signature);
    emit_placement_trace(
        PlacementTracePhase::Preview,
        world,
        footprint_catalog,
        definition,
        candidate_anchor,
        yaw_degrees,
        layout,
        vertical_scale,
        validation,
        plan,
    );
}

pub fn trace_build_mode_place_result(
    trace: &BuildModePlacementTrace,
    world: &WorldData,
    footprint_catalog: &FootprintCatalog,
    definition: &BuildingDefinition,
    candidate_anchor: WorldPosition,
    yaw_degrees: f32,
    layout: ChunkLayout,
    vertical_scale: f32,
    validation: &BuildingPlacementValidation,
    plan: Option<&BuildingPlacementPlan>,
    place_result: &Result<BuildingRecord, BuildingAuthoringError>,
) {
    if !trace.enabled {
        return;
    }
    emit_placement_trace(
        PlacementTracePhase::Commit,
        world,
        footprint_catalog,
        definition,
        candidate_anchor,
        yaw_degrees,
        layout,
        vertical_scale,
        validation,
        plan,
    );
    match place_result {
        Ok(record) => {
            let exists = world.get_building(record.id).is_some();
            let global = record.placement.position.to_global(layout);
            let terrain_render = terrain_surface_render_y_at(
                global.x,
                global.z,
                world,
                layout,
                vertical_scale,
            );
            let building_render = building_placement_render_y(global.y, vertical_scale);
            info!(
                "FINAL:\n  building placed: yes\n  building_id: {}\n  building render y: {:.3}\n  terrain render y at anchor: {}\n  foundation entity expected: {}\n  inventory allocated: {}\n  WorldData record exists: {}",
                record.id.raw(),
                building_render,
                terrain_render
                    .map(|y| format!("{:.3}", y))
                    .unwrap_or_else(|| "unavailable".to_string()),
                if definition.terrain_placement_mode == TerrainPlacementMode::LevelFoundation {
                    "yes when skirt spec present"
                } else {
                    "no"
                },
                if record.inventory_id.is_some() { "yes" } else { "no" },
                if exists { "yes" } else { "no" }
            );
            info!(
                "PLACE RESULT:\n  success: yes\n  building_id: {}\n  inventory allocated: {}\n  occupancy registered: yes\n  WorldData record exists: {}\n  render entity expected: yes\n  rollback performed: no",
                record.id.raw(),
                if record.inventory_id.is_some() { "yes" } else { "no" },
                if exists { "yes" } else { "no" }
            );
        }
        Err(error) => {
            let (allocated_id, rollback) = match error {
                BuildingAuthoringError::InventoryAllocationFailed(id) => {
                    (Some(id.raw()), world.get_building(*id).is_none())
                }
                BuildingAuthoringError::Occupancy(_) => (None, true),
                _ => (None, false),
            };
            info!(
                "PLACE RESULT:\n  success: no\n  building_id: {}\n  inventory allocated: no\n  occupancy registered: no\n  WorldData record exists: no\n  render entity expected: no\n  rollback performed: {}\n  error: {:?}",
                allocated_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                if rollback { "yes" } else { "no" },
                error,
            );
        }
    }
}

fn preview_signature(
    layout: ChunkLayout,
    anchor: WorldPosition,
    yaw_degrees: f32,
    validation: &BuildingPlacementValidation,
    plan: Option<&BuildingPlacementPlan>,
) -> u64 {
    let g = anchor.to_global(layout);
    let reason = validation.primary_reason.map(|r| r as u8).unwrap_or(255) as u64;
    let valid = if validation.valid { 1u64 } else { 0 };
    let cell_hash = plan
        .map(|p| p.occupied_cells.len() as u64)
        .unwrap_or(0);
    (g.x.to_bits() as u64)
        ^ (g.z.to_bits() as u64)
        ^ (yaw_degrees.to_bits() as u64)
        ^ (reason << 8)
        ^ (valid << 16)
        ^ (cell_hash << 24)
}

fn emit_placement_trace(
    phase: PlacementTracePhase,
    world: &WorldData,
    footprint_catalog: &FootprintCatalog,
    definition: &BuildingDefinition,
    candidate_anchor: WorldPosition,
    yaw_degrees: f32,
    layout: ChunkLayout,
    vertical_scale: f32,
    validation: &BuildingPlacementValidation,
    plan: Option<&BuildingPlacementPlan>,
) {
    let candidate_global = candidate_anchor.to_global(layout);
    let grounded = validation.grounded_anchor.map(|p| p.to_global(layout));
    let resolved_rotation = validation.resolved_rotation;
    let foundation = validation.foundation.as_ref();

    info!("--- BUILD PLACEMENT TRACE ({}) ---", phase_label(phase));

    info!(
        "BUILDING:\n  building_key: {}\n  terrain_placement_mode: {}",
        definition.id.as_str(),
        definition.terrain_placement_mode.label()
    );

    info!(
        "CANDIDATE:\n  world x/z: {:.3}, {:.3}\n  candidate y: {:.3}\n  yaw degrees: {:.1}",
        candidate_global.x,
        candidate_global.z,
        candidate_global.y,
        yaw_degrees
    );

    if let Some(plan) = plan {
        info!(
            "FOOTPRINT:\n  occupied cells: {}\n  anchor_global_xz: {:.3}, {:.3}\n  quantized yaw: {}°",
            plan.occupied_cells.len(),
            plan.anchor_global_xz.x,
            plan.anchor_global_xz.y,
            plan.quantized_rotation.degrees()
        );
    }

    if let (Some(grounded_global), Some(rotation)) = (grounded, resolved_rotation) {
        let (pitch, yaw, roll) = rotation.to_euler(EulerRot::YXZ);
        info!(
            "PLAN:\n  resolved position: {:.3}, {:.3}, {:.3}\n  resolved quaternion: ({:.4}, {:.4}, {:.4}, {:.4})\n  resolved pitch: {:.2}°\n  resolved roll: {:.2}°\n  resolved yaw: {:.2}°",
            grounded_global.x,
            grounded_global.y,
            grounded_global.z,
            rotation.x,
            rotation.y,
            rotation.z,
            rotation.w,
            pitch.to_degrees(),
            roll.to_degrees(),
            yaw.to_degrees()
        );
    }

    if let (Some(grounded_global), Ok(shape)) = (
        grounded,
        effective_building_footprint_for_placement(definition, footprint_catalog, 1.0),
    ) {
        let anchor_xz = Vec2::new(grounded_global.x, grounded_global.z);
        let yaw_radians = QuantizedRotation::from_degrees_snapped(yaw_degrees)
            .map(|q| q.radians())
            .unwrap_or(yaw_degrees.to_radians());
        if let Ok(report) = sample_terrain_under_footprint(
            world,
            layout,
            shape.as_ref(),
            anchor_xz,
            yaw_radians,
        ) {
            let sim_slope = slope_degrees_from_plane_coefficients(report.plane_a, report.plane_b);
            let pres_slope = slope_degrees_from_plane_coefficients(
                report.plane_a * vertical_scale,
                report.plane_b * vertical_scale,
            );
            let pres_normal = presentation_plane_normal(&report, vertical_scale);
            let delta_h = report.max_height - report.min_height;
            info!(
                "TERRAIN:\n  sample count: {}\n  min height: {:.3}\n  max height: {:.3}\n  height range: {:.3}\n  simulation slope: {:.2}° (Δh={:.3})\n  presentation slope: {:.2}° (Δh_render≈{:.3})\n  simulation normal: {:.3}, {:.3}, {:.3}\n  presentation normal: {:.3}, {:.3}, {:.3}\n  RMS residual: {:.4}\n  peak residual: {:.4}",
                report.samples.len(),
                report.min_height,
                report.max_height,
                report.height_range,
                sim_slope,
                delta_h,
                pres_slope,
                delta_h * vertical_scale,
                report.plane_normal.x,
                report.plane_normal.y,
                report.plane_normal.z,
                pres_normal.x,
                pres_normal.y,
                pres_normal.z,
                report.plane_rms_residual,
                report.plane_peak_residual
            );

            if definition.terrain_placement_mode == TerrainPlacementMode::ConformToTerrain {
                if let Some(rotation) = resolved_rotation {
                    let up = rotation * Vec3::Y;
                    let resolved_tilt = up.angle_between(Vec3::Y).to_degrees();
                    let terrain_tilt = pres_normal.angle_between(Vec3::Y).to_degrees();
                    info!(
                        "CONFORM:\n  resolved up: {:.3}, {:.3}, {:.3}\n  terrain presentation tilt: {:.2}°\n  resolved up-vector tilt: {:.2}°\n  clearance lift anchor y: {:.3}",
                        up.x,
                        up.y,
                        up.z,
                        terrain_tilt,
                        resolved_tilt,
                        grounded_global.y
                    );
                }
            }
        }
    }

    if let Some(grounded_global) = grounded {
        let terrain_sim = terrain_surface_sim_y_at(
            grounded_global.x,
            grounded_global.z,
            world,
            layout,
        );
        let terrain_render = terrain_surface_render_y_at(
            grounded_global.x,
            grounded_global.z,
            world,
            layout,
            vertical_scale,
        );
        info!(
            "AUTHORITATIVE:\n  simulation placement y: {:.6}\n  simulation terrain y at anchor: {}",
            grounded_global.y,
            terrain_sim
                .map(|y| format!("{:.6}", y))
                .unwrap_or_else(|| "unavailable".to_string())
        );
        info!(
            "PRESENTATION:\n  terrain rendered y at anchor: {}\n  building render y: {:.3}\n  vertical scale: {:.3}",
            terrain_render
                .map(|y| format!("{:.3}", y))
                .unwrap_or_else(|| "unavailable".to_string()),
            building_placement_render_y(grounded_global.y, vertical_scale),
            vertical_scale
        );
    }

    if definition.terrain_placement_mode == TerrainPlacementMode::LevelFoundation {
        if let (Some(spec), Some(_plan)) = (foundation, plan) {
            let top_span = spec
                .perimeter
                .iter()
                .map(|v| v.local_xz)
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(min_x, max_x), p| {
                    (min_x.min(p.x), max_x.max(p.x))
                });
            info!(
                "LEVEL FOUNDATION:\n  presentation terrain min/max beneath footprint: sim depth {:.3} / presentation depth {:.3}\n  presentation foundation depth: {:.3}\n  foundation slope degrees: {:.1}\n  top perimeter x span: {:.3}\n  outward expansion: {:.3}\n  FoundationSkirtSpec present: yes\n  perimeter vertices: {}",
                spec.max_depth_meters,
                spec.presentation_depth_meters,
                spec.presentation_depth_meters,
                FOUNDATION_SLOPE_DEGREES,
                top_span.1 - top_span.0,
                spec.outward_expansion_meters,
                spec.perimeter.len()
            );
        } else if let (Some(grounded_global), Ok(shape)) = (
            grounded,
            effective_building_footprint_for_placement(definition, footprint_catalog, 1.0),
        ) {
            let anchor_xz = Vec2::new(grounded_global.x, grounded_global.z);
            let yaw_radians = QuantizedRotation::from_degrees_snapped(yaw_degrees)
                .map(|q| q.radians())
                .unwrap_or(yaw_degrees.to_radians());
            if let Ok(report) = sample_terrain_under_footprint(
                world,
                layout,
                shape.as_ref(),
                anchor_xz,
                yaw_radians,
            ) {
                let span = footprint_horizontal_span_meters(shape.as_ref());
                let presentation_depth =
                    presentation_foundation_depth_meters(&report, span, vertical_scale);
                info!(
                    "LEVEL FOUNDATION:\n  floor_y: {:.3}\n  simulation range: {:.3}\n  presentation depth: {:.3}\n  FoundationSkirtSpec present: no",
                    grounded_global.y,
                    report.height_range,
                    presentation_depth
                );
            }
        }
    }

    if validation.valid {
        info!("VALIDATION: valid");
    } else {
        let reason = validation
            .primary_reason
            .map(BuildingPlacementRejectReason::label)
            .unwrap_or("unknown");
        info!(
            "VALIDATION: invalid\n  rejection reason: {}\n  all reasons: {}",
            reason,
            validation
                .reasons
                .iter()
                .map(|r| r.label())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if let (Some(_grounded_global), Some(rotation)) = (grounded, resolved_rotation) {
        if definition.terrain_placement_mode == TerrainPlacementMode::ConformToTerrain {
            let up = rotation * Vec3::Y;
            let resolved_tilt = up.angle_between(Vec3::Y).to_degrees();
            info!(
                "CONFORM TILT:\n  terrain presentation tilt: see TERRAIN block above\n  resolved building tilt: {:.2}°\n  foundation skirt: {}",
                resolved_tilt,
                if foundation.is_some() { "spec yes" } else { "spec no" }
            );
        }
    }
}

/// Runs after [`super::preview::update_build_mode_ghost`] to emit change-only preview traces.
pub fn trace_build_mode_preview_after_update(
    mut trace: ResMut<BuildModePlacementTrace>,
    build_mode: Res<super::state::BuildModeState>,
    world: Res<WorldData>,
    config: Res<crate::world::WorldConfig>,
    render_assets: Option<Res<crate::terrain::TerrainRenderAssets>>,
    building_catalog: Res<crate::world::BuildingCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
) {
    if !build_mode.is_ghost_placing() {
        return;
    }
    let Some(definition_id) = build_mode.ghost_definition_id() else {
        return;
    };
    let Some(definition) = building_catalog.get(definition_id) else {
        return;
    };
    let Some(validation) = build_mode.last_validation.as_ref() else {
        return;
    };
    let Some(anchor) = build_mode
        .last_plan
        .as_ref()
        .map(|plan| plan.grounded_anchor)
        .or_else(|| validation.grounded_anchor)
    else {
        return;
    };
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|a| a.vertical_scale)
        .unwrap_or(1.0);
    let yaw_degrees = (build_mode.ghost_rotation_quadrants() % 4) as f32 * 90.0;
    maybe_trace_build_mode_preview(
        &mut trace,
        &world,
        &footprint_catalog,
        definition,
        anchor,
        yaw_degrees,
        layout,
        vertical_scale,
        validation,
        build_mode.last_plan.as_ref(),
    );
}

fn phase_label(phase: PlacementTracePhase) -> &'static str {
    match phase {
        PlacementTracePhase::Preview => "preview",
        PlacementTracePhase::Commit => "commit",
    }
}
