//! Authoritative footprint resolution for selection presentation (Slice 2).

use bevy::prelude::*;

use crate::item_piles::ItemPilePresentationSettings;
use crate::terrain::world_position_to_render_global;
use crate::world::{
    BuildingCatalog, BuildingId, BuildingRecord, DoodadCatalog, DoodadId, DoodadRecord,
    FootprintCatalog, ItemPileId, ItemPileSettings, WorldData, WorldItemPileRecord,
};
use crate::world::{
    DoodadInstanceCollision, FootprintShape, doodad_interaction_radius_meters,
    effective_building_footprint_for_placement, occupied_cells_for_footprint_yaw,
    resolve_doodad_collision_from_catalog,
};

/// Minimum selection ring radius for item piles (presentation-only).
pub const ITEM_PILE_SELECTION_MIN_RADIUS_METERS: f32 = 0.45;

/// Resolved horizontal footprint for one selected world object.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSelectionFootprint {
    pub anchor_render: Vec3,
    pub yaw_radians: f32,
    pub shape: FootprintShape,
    /// When true, ring vertices sample terrain height (units, ground piles).
    pub terrain_conforming: bool,
}

/// Presentation targets for non-unit selection (at most one active).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldObjectPresentationTarget {
    Building(BuildingId),
    Doodad(DoodadId),
    ItemPile(ItemPileId),
}

impl WorldObjectPresentationTarget {
    pub fn from_selection(
        category: super::super::WorldSelectionCategory,
        building_id: Option<BuildingId>,
        doodad_id: Option<DoodadId>,
        pile_id: Option<ItemPileId>,
    ) -> Option<Self> {
        use super::super::WorldSelectionCategory;
        match category {
            WorldSelectionCategory::Building => building_id.map(Self::Building),
            WorldSelectionCategory::Doodad => doodad_id.map(Self::Doodad),
            WorldSelectionCategory::ItemPile => pile_id.map(Self::ItemPile),
            _ => None,
        }
    }
}

/// Resolve building authoritative footprint for selection outline.
pub fn resolve_building_selection_footprint(
    record: &BuildingRecord,
    definition: &crate::world::BuildingDefinition,
    footprint_catalog: &FootprintCatalog,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> Option<ResolvedSelectionFootprint> {
    let shape = effective_building_footprint_for_placement(
        definition,
        footprint_catalog,
        record.placement.uniform_scale_f32(),
    )
    .ok()?;
    let anchor_render =
        world_position_to_render_global(record.placement.position, layout, vertical_scale);
    let yaw = record.placement.rotation.to_euler(EulerRot::YXZ).0;
    Some(ResolvedSelectionFootprint {
        anchor_render,
        yaw_radians: yaw,
        shape: shape.into_owned(),
        terrain_conforming: false,
    })
}

/// Resolve doodad collision footprint; non-blockers use interaction radius.
pub fn resolve_doodad_selection_footprint_with_collision(
    record: &DoodadRecord,
    definition: &crate::world::DoodadDefinition,
    collision: &DoodadInstanceCollision,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> Option<ResolvedSelectionFootprint> {
    let anchor_render =
        world_position_to_render_global(record.placement.position, layout, vertical_scale);
    let mut shape = collision.shape.clone();
    if !collision.blocks_movement || shape_is_empty(&shape) {
        let radius = doodad_interaction_radius_meters(record, definition);
        if radius <= 0.0 {
            return None;
        }
        shape = FootprintShape::Circle {
            radius_meters: radius,
        };
    }
    Some(ResolvedSelectionFootprint {
        anchor_render,
        yaw_radians: collision.yaw_radians,
        shape,
        terrain_conforming: false,
    })
}

/// Item piles use a small terrain-conforming ring at the pile anchor.
pub fn resolve_item_pile_selection_footprint(
    record: &WorldItemPileRecord,
    pile_settings: &ItemPileSettings,
    presentation: &ItemPilePresentationSettings,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> ResolvedSelectionFootprint {
    let anchor_render = world_position_to_render_global(record.placement, layout, vertical_scale);
    let authored =
        (pile_settings.merge_radius_meters * 0.35).min(presentation.fallback_sphere_radius * 0.65);
    let radius = authored.max(ITEM_PILE_SELECTION_MIN_RADIUS_METERS);
    ResolvedSelectionFootprint {
        anchor_render,
        yaw_radians: 0.0,
        shape: FootprintShape::Circle {
            radius_meters: radius,
        },
        terrain_conforming: true,
    }
}

/// Lookup footprint for the active world-object selection target.
pub fn resolve_world_object_footprint(
    target: WorldObjectPresentationTarget,
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    doodad_catalog: &DoodadCatalog,
    pile_settings: &ItemPileSettings,
    pile_presentation: &ItemPilePresentationSettings,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) -> Option<ResolvedSelectionFootprint> {
    match target {
        WorldObjectPresentationTarget::Building(id) => {
            let record = world.get_building(id)?;
            let definition = building_catalog.get(&record.definition_id)?;
            resolve_building_selection_footprint(
                record,
                definition,
                footprint_catalog,
                layout,
                vertical_scale,
            )
        }
        WorldObjectPresentationTarget::Doodad(id) => {
            let record = world.get_doodad(id)?;
            let definition = doodad_catalog.get(&record.definition_id)?;
            let collision = resolve_doodad_collision_from_catalog(record, doodad_catalog);
            resolve_doodad_selection_footprint_with_collision(
                record,
                definition,
                &collision,
                layout,
                vertical_scale,
            )
        }
        WorldObjectPresentationTarget::ItemPile(id) => {
            let record = world.item_pile_store().get(id)?;
            Some(resolve_item_pile_selection_footprint(
                record,
                pile_settings,
                pile_presentation,
                layout,
                vertical_scale,
            ))
        }
    }
}

fn shape_is_empty(shape: &FootprintShape) -> bool {
    match shape {
        FootprintShape::Circle { radius_meters } => *radius_meters <= 0.0,
        FootprintShape::Ellipse {
            radius_x_meters,
            radius_z_meters,
        } => *radius_x_meters <= 0.0 && *radius_z_meters <= 0.0,
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => *width_meters <= 0.0 || *depth_meters <= 0.0,
        FootprintShape::BakedCellMask(_) => false,
    }
}

/// Occupied cells for a resolved footprint (used by outline mesh and tests).
pub fn occupied_cells_for_resolved(
    footprint: &ResolvedSelectionFootprint,
) -> Vec<crate::world::OccupancyCellCoord> {
    let anchor_xz = Vec2::new(footprint.anchor_render.x, footprint.anchor_render.z);
    occupied_cells_for_footprint_yaw(&footprint.shape, anchor_xz, footprint.yaw_radians)
}

#[cfg(test)]
mod tests;
