//! Terrain field query API (ADR-101).

use bevy::prelude::*;

use super::catalog::TerrainFieldCatalog;
use super::compose::compose_terrain_field_value;
use super::interpolate::bilinear_sample_u16;
use super::mapping::{FieldMappingError, world_position_to_field_local};
use super::region::FieldSampleRegion;
use super::sample::{
    FieldAreaAvailability, FieldAvailability, TerrainFieldAreaReport, TerrainFieldSample,
};
use super::store::TerrainFieldStore;
use super::{BasisPoints, TerrainFieldId};
use crate::world::data::ChunkExtent;
use crate::world::occupancy::OccupancyCellCoord;
use crate::world::{ChunkLayout, WorldData, WorldPosition};

/// Sample one terrain field at an authoritative world position.
pub fn sample_terrain_field_at(
    world: &WorldData,
    catalog: &TerrainFieldCatalog,
    field_id: &TerrainFieldId,
    position: WorldPosition,
) -> TerrainFieldSample {
    let Some(definition) = catalog.get(field_id) else {
        return TerrainFieldSample::unavailable(
            field_id.clone(),
            FieldAvailability::FieldDefinitionMissing,
        );
    };
    if !definition.enabled {
        return TerrainFieldSample::unavailable(field_id.clone(), FieldAvailability::FieldDisabled);
    }
    if let Some(extent) = world.extent() {
        if !extent_contains_position(extent, position, world.layout()) {
            return TerrainFieldSample::unavailable(
                field_id.clone(),
                FieldAvailability::OutsideWorld,
            );
        }
    }
    let layout = world.layout();
    let (_, local_coord) = match world_position_to_field_local(position, layout) {
        Ok(value) => value,
        Err(FieldMappingError::OutsideChunkDomain) => {
            return TerrainFieldSample::unavailable(
                field_id.clone(),
                FieldAvailability::InvalidCoordinate,
            );
        }
    };
    let chunk = position.chunk;
    let Some(tile) = world.terrain_fields().get_tile(field_id, chunk) else {
        return TerrainFieldSample::unavailable(field_id.clone(), FieldAvailability::TileMissing);
    };
    let (value, _) = match bilinear_sample_u16(tile, local_coord) {
        Ok(result) => result,
        Err(_) => {
            return TerrainFieldSample::unavailable(
                field_id.clone(),
                FieldAvailability::CorruptTile,
            );
        }
    };
    TerrainFieldSample::available(
        field_id.clone(),
        compose_terrain_field_value(value, field_id, chunk, world.terrain_field_modifiers()),
        chunk,
        tile.tile_revision,
    )
}

/// Sample a terrain field across a deterministic occupancy-cell region.
pub fn sample_terrain_field_area(
    world: &WorldData,
    catalog: &TerrainFieldCatalog,
    field_id: &TerrainFieldId,
    region: &FieldSampleRegion,
    usable_threshold: u16,
) -> TerrainFieldAreaReport {
    if region.is_empty() {
        return TerrainFieldAreaReport::empty_region(field_id.clone());
    }
    let mut sum: u64 = 0;
    let mut available_cells = 0u32;
    let mut unavailable_cells = 0u32;
    let mut min_value = u16::MAX;
    let mut max_value = 0u16;
    let mut usable_count = 0u32;
    let layout = world.layout();

    for cell in region.cells() {
        let center = cell.center_global();
        let position = WorldPosition::from_global(Vec3::new(center.x, 0.0, center.y), layout);
        let sample = sample_terrain_field_at(world, catalog, field_id, position);
        if sample.availability.is_available() {
            available_cells += 1;
            sum += sample.value as u64;
            min_value = min_value.min(sample.value);
            max_value = max_value.max(sample.value);
            if sample.value >= usable_threshold {
                usable_count += 1;
            }
        } else {
            unavailable_cells += 1;
        }
    }

    let requested = region.len() as u32;
    let average = if available_cells > 0 {
        Some(((sum + available_cells as u64 / 2) / available_cells as u64) as u16)
    } else {
        None
    };
    let minimum = if available_cells > 0 {
        Some(min_value)
    } else {
        None
    };
    let maximum = if available_cells > 0 {
        Some(max_value)
    } else {
        None
    };
    let usable_coverage = if available_cells > 0 {
        BasisPoints::from_ratio(usable_count, available_cells).unwrap_or(BasisPoints::ZERO)
    } else {
        BasisPoints::ZERO
    };
    let availability = if available_cells == 0 {
        FieldAreaAvailability::NoneAvailable
    } else if unavailable_cells > 0 {
        FieldAreaAvailability::PartiallyAvailable
    } else {
        FieldAreaAvailability::AllAvailable
    };

    TerrainFieldAreaReport {
        field_id: field_id.clone(),
        requested_cells: requested,
        available_cells,
        unavailable_cells,
        average,
        minimum,
        maximum,
        usable_coverage,
        availability,
    }
}

/// Build a [`FieldSampleRegion`] from occupancy cells in canonical sorted order.
pub fn field_sample_region_from_cells(cells: Vec<OccupancyCellCoord>) -> FieldSampleRegion {
    FieldSampleRegion::from_cells(cells)
}

fn extent_contains_position(
    extent: ChunkExtent,
    position: WorldPosition,
    layout: ChunkLayout,
) -> bool {
    let chunk = position.chunk;
    chunk.x >= extent.min.x
        && chunk.z >= extent.min.z
        && chunk.x <= extent.max.x
        && chunk.z <= extent.max.z
        && position.local.0.x >= -1e-4
        && position.local.0.z >= -1e-4
        && position.local.0.x <= layout.chunk_size_units() + 1e-4
        && position.local.0.z <= layout.chunk_size_units() + 1e-4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain_field::fixtures::{
        bootstrap_constant_field, bootstrap_x_gradient_field,
    };
    use crate::world::terrain_field::store::TerrainFieldStore;
    use crate::world::{ChunkCoord, LocalPosition, WorldConfig};

    fn test_world_with_field(field_id: &str, value: u16) -> (WorldData, TerrainFieldCatalog) {
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        });
        bootstrap_constant_field(
            world.terrain_fields_mut(),
            TerrainFieldId::new(field_id),
            ChunkCoord::new(0, 0),
            value,
        );
        let catalog = TerrainFieldCatalog::from_definitions(initial_test_definitions()).unwrap();
        (world, catalog)
    }

    fn initial_test_definitions() -> Vec<super::super::definition::TerrainFieldDefinition> {
        crate::world::terrain_field::catalog::starter::starter_definitions()
    }

    #[test]
    fn constant_field_point_query() {
        let (world, catalog) = test_world_with_field("water", 30_000);
        let pos = WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::ZERO));
        let sample = sample_terrain_field_at(&world, &catalog, &TerrainFieldId::new("water"), pos);
        assert!(sample.availability.is_available());
        assert_eq!(sample.value, 30_000);
    }

    #[test]
    fn missing_tile_is_not_zero() {
        let (world, catalog) = test_world_with_field("water", 30_000);
        let pos = WorldPosition::new(ChunkCoord::new(5, 5), LocalPosition::new(Vec3::ZERO));
        let sample = sample_terrain_field_at(&world, &catalog, &TerrainFieldId::new("water"), pos);
        assert!(!sample.availability.is_available());
        assert_eq!(sample.value, 0);
    }

    #[test]
    fn x_gradient_moves_with_x() {
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(0, 0),
        });
        bootstrap_x_gradient_field(
            world.terrain_fields_mut(),
            TerrainFieldId::new("iron"),
            ChunkCoord::new(0, 0),
        );
        let catalog = TerrainFieldCatalog::from_definitions(initial_test_definitions()).unwrap();
        let low = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(0.0, 0.0, 64.0)),
        );
        let high = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(128.0, 0.0, 64.0)),
        );
        let s_low = sample_terrain_field_at(&world, &catalog, &TerrainFieldId::new("iron"), low);
        let s_high = sample_terrain_field_at(&world, &catalog, &TerrainFieldId::new("iron"), high);
        assert!(s_high.value > s_low.value);
    }
}
