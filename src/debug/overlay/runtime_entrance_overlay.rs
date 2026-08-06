//! Runtime entrance portal overlay (IN-11eO) — activated portal registry, not authored glyphs.

use bevy::prelude::*;

use super::helpers::render_position;
use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::debug::settings::DebugOverlaySettings;
use crate::terrain::TerrainRenderAssets;
use crate::units::input::SelectedUnits;
use crate::world::{
    BuildingNavigationRuntime, ChunkLayout, PortalType, SpaceId, WorldConfig, WorldData,
};

const SPECIALIZED_OVERLAY_RADIUS_METERS: f32 = 96.0;
const Y_LIFT_SURFACE: f32 = 0.14;
const Y_LIFT_INTERIOR: f32 = 0.18;

/// Draw runtime portal triggers for the selected building.
pub fn draw_runtime_entrance_overlay(
    mut gizmos: Gizmos,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    settings: Res<DebugOverlaySettings>,
    world_selection: Res<WorldSelectionState>,
    selection: Res<SelectedUnits>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.category_enabled(crate::debug::settings::DebugOverlayCategory::NavEntrances) {
        return;
    }

    let building_id = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten();
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    let focus = debug_focus(&world, &selection, &world_selection, layout);
    let radius_sq = SPECIALIZED_OVERLAY_RADIUS_METERS * SPECIALIZED_OVERLAY_RADIUS_METERS;

    let portals: Vec<_> = if let Some(building_id) = building_id {
        world
            .space_registry()
            .portals()
            .filter(|(_, portal)| portal.owning_building_id == Some(building_id))
            .map(|(id, portal)| (*id, portal.clone()))
            .collect()
    } else {
        world
            .space_registry()
            .portals()
            .filter(|(_, portal)| {
                portal.portal_type == PortalType::ExteriorEntrance
                    || portal.portal_type == PortalType::Doorway
            })
            .map(|(id, portal)| (*id, portal.clone()))
            .collect()
    };

    for (portal_id, portal) in portals {
        let center_xz = portal.from_center_global_xz;
        let dx = center_xz.x - focus.x;
        let dz = center_xz.y - focus.z;
        let in_range = dx * dx + dz * dz <= radius_sq;

        let portal_key = building_id
            .and_then(|id| world.building_navigation_runtime().get(id))
            .and_then(|runtime| portal_key_for(runtime, portal_id));

        let (ring, fill, line) =
            runtime_portal_colors(portal.portal_type, portal.enabled, in_range);
        let surface_y = portal_surface_y(&world, center_xz, layout, vertical_scale);
        let surface_center = Vec3::new(center_xz.x, surface_y, center_xz.y);

        if in_range {
            draw_portal_threshold(
                &mut gizmos,
                surface_center,
                portal.from_radius_meters,
                ring,
                fill,
            );
            let dest = render_position(portal.to_position, layout, vertical_scale);
            let interior_y = dest.y + Y_LIFT_INTERIOR * vertical_scale;
            let dest_render = Vec3::new(dest.x, interior_y, dest.z);
            gizmos.line(surface_center, dest_render, line);
            gizmos.sphere(dest_render, portal.from_radius_meters * 0.12, fill);
        }

        let _ = portal_key;
    }
}

fn portal_key_for(
    runtime: &BuildingNavigationRuntime,
    portal_id: crate::world::PortalId,
) -> Option<&str> {
    runtime
        .portal_keys
        .iter()
        .find(|(_, id)| **id == portal_id)
        .map(|(key, _)| key.as_str())
}

fn portal_surface_y(
    world: &WorldData,
    center_xz: Vec2,
    layout: ChunkLayout,
    vertical_scale: f32,
) -> f32 {
    super::nav_cells::sample_terrain_y(world, center_xz, layout, vertical_scale) + Y_LIFT_SURFACE
}

fn draw_portal_threshold(gizmos: &mut Gizmos, center: Vec3, radius: f32, ring: Color, fill: Color) {
    gizmos.circle(Isometry3d::new(center, Quat::IDENTITY), radius, ring);
    gizmos.sphere(center, radius * 0.1, fill);
    let tangent = Vec3::new(1.0, 0.0, 0.0) * radius * 1.6;
    gizmos.line(center - tangent, center + tangent, ring);
}

fn runtime_portal_colors(
    portal_type: PortalType,
    enabled: bool,
    in_range: bool,
) -> (Color, Color, Color) {
    let alpha_scale = if enabled { 1.0 } else { 0.35 };
    let range_scale = if in_range { 1.0 } else { 0.45 };
    let a = alpha_scale * range_scale;
    match portal_type {
        PortalType::ExteriorEntrance => (
            Color::srgba(0.1, 0.95, 0.85, 0.9 * a),
            Color::srgba(0.1, 0.95, 0.85, 0.35 * a),
            Color::srgba(0.2, 0.85, 1.0, 0.75 * a),
        ),
        PortalType::Doorway => (
            Color::srgba(0.45, 0.7, 1.0, 0.85 * a),
            Color::srgba(0.45, 0.7, 1.0, 0.3 * a),
            Color::srgba(0.55, 0.75, 1.0, 0.65 * a),
        ),
        PortalType::Stair | PortalType::Ramp => (
            Color::srgba(0.75, 0.5, 1.0, 0.8 * a),
            Color::srgba(0.75, 0.5, 1.0, 0.3 * a),
            Color::srgba(0.7, 0.55, 0.95, 0.6 * a),
        ),
        PortalType::CaveEntrance => (
            Color::srgba(0.55, 0.4, 0.3, 0.8 * a),
            Color::srgba(0.55, 0.4, 0.3, 0.3 * a),
            Color::srgba(0.5, 0.45, 0.35, 0.55 * a),
        ),
    }
}

fn debug_focus(
    world: &WorldData,
    selection: &SelectedUnits,
    world_selection: &WorldSelectionState,
    layout: ChunkLayout,
) -> Vec3 {
    if let Some(unit_id) = world_selection
        .primary_unit(selection)
        .or_else(|| selection.iter().next())
    {
        if let Some(unit) = world.get_unit(unit_id) {
            return unit.placement.position.to_global(layout);
        }
    }
    if let Some(building_id) = (world_selection.category == WorldSelectionCategory::Building)
        .then_some(world_selection.building_id)
        .flatten()
    {
        if let Some(building) = world.get_building(building_id) {
            return building.placement.position.to_global(layout);
        }
    }
    Vec3::ZERO
}
