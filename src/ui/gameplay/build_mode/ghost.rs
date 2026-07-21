//! Build footprint ghost presentation (ADR-081 B4).

use bevy::prelude::*;

use crate::terrain::TerrainRenderAssets;
use crate::world::{
    BuildingCatalog, BuildingPlacementRejectReason, FootprintCatalog, QuantizedRotation,
    WorldConfig, WorldData, effective_building_footprint_for_placement, occupied_cells_for_footprint,
    rotation_from_quadrants,
};

use super::state::BuildModeState;

/// Ghost validity category for coloring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildGhostStatus {
    #[default]
    Unavailable,
    Valid,
    StaticBlocked,
    UnitBlocked,
    TerrainInvalid,
}

impl BuildGhostStatus {
    pub fn from_reason(reason: BuildingPlacementRejectReason) -> Self {
        match reason {
            BuildingPlacementRejectReason::OccupiedByUnit => Self::UnitBlocked,
            BuildingPlacementRejectReason::OccupiedByBuilding
            | BuildingPlacementRejectReason::OccupiedByDoodad => Self::StaticBlocked,
            BuildingPlacementRejectReason::TerrainUnavailable
            | BuildingPlacementRejectReason::SlopeTooSteep
            | BuildingPlacementRejectReason::HeightVariationTooLarge => Self::TerrainInvalid,
            _ => Self::Unavailable,
        }
    }

    pub fn color(self) -> Color {
        match self {
            Self::Valid => Color::srgba(0.2, 0.9, 0.35, 0.55),
            Self::StaticBlocked => Color::srgba(0.95, 0.2, 0.2, 0.6),
            Self::UnitBlocked => Color::srgba(0.95, 0.65, 0.15, 0.6),
            Self::TerrainInvalid => Color::srgba(0.9, 0.2, 0.2, 0.5),
            Self::Unavailable => Color::srgba(0.55, 0.55, 0.55, 0.45),
        }
    }
}

/// Draw client-local building placement ghost (gizmos only).
pub fn draw_build_mode_ghost(
    build_mode: Res<BuildModeState>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    building_catalog: Res<BuildingCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut gizmos: Gizmos,
) {
    if !build_mode.is_ghost_placing() {
        return;
    }
    let Some(validation) = &build_mode.last_validation else {
        return;
    };
    let Some(anchor) = validation.grounded_anchor else {
        return;
    };
    let Some(definition_id) = build_mode.ghost_definition_id() else {
        return;
    };
    let Some(definition) = building_catalog.get(definition_id) else {
        return;
    };
    let Ok(shape) = effective_building_footprint_for_placement(definition, &footprint_catalog, 1.0)
    else {
        return;
    };

    let status = if validation.valid {
        BuildGhostStatus::Valid
    } else if let Some(reason) = validation.primary_reason {
        BuildGhostStatus::from_reason(reason)
    } else {
        BuildGhostStatus::Unavailable
    };
    let color = status.color();
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let anchor_global = anchor.to_global(layout);
    let anchor_xz = Vec2::new(anchor_global.x, anchor_global.z);
    let rotation = QuantizedRotation::from_quat(rotation_from_quadrants(
        build_mode.ghost_rotation_quadrants(),
    ))
    .unwrap_or(QuantizedRotation::Deg0);
    let cells = build_mode
        .last_plan
        .as_ref()
        .map(|plan| plan.occupied_cells.clone())
        .unwrap_or_else(|| occupied_cells_for_footprint(shape.as_ref(), anchor_xz, rotation));
    let cell_size = crate::world::OCCUPANCY_CELL_SIZE_METERS;

    for cell in cells {
        let center = cell.center_global();
        let y = sample_render_y(&world, center, layout, vertical_scale);
        let half = cell_size * 0.48;
        let corners = [
            Vec3::new(center.x - half, y + 0.05, center.y - half),
            Vec3::new(center.x + half, y + 0.05, center.y - half),
            Vec3::new(center.x + half, y + 0.05, center.y + half),
            Vec3::new(center.x - half, y + 0.05, center.y + half),
        ];
        for i in 0..4 {
            gizmos.line(corners[i], corners[(i + 1) % 4], color);
        }
    }

    gizmos.sphere(
        Vec3::new(
            anchor_global.x,
            anchor_global.y * vertical_scale + 0.1,
            anchor_global.z,
        ),
        0.35,
        color,
    );
}

fn sample_render_y(
    world: &WorldData,
    center: Vec2,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> f32 {
    use crate::world::{ChunkCoord, LocalPosition, WorldPosition, ground_world_position};
    let sample = WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(center.x, 0.0, center.y)),
    );
    let corrected = WorldPosition::from_global(Vec3::new(center.x, 0.0, center.y), layout);
    ground_world_position(world, corrected)
        .or_else(|| ground_world_position(world, sample))
        .map(|p| p.to_global(layout).y * vertical_scale)
        .unwrap_or(0.0)
}
