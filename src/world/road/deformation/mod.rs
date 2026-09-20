//! Derived per-chunk road terrain deformation (effective height deltas).

mod bake;
mod persist;
mod refresh;
mod store;
mod tile;

pub use bake::{
    ROAD_BAKE_INFLUENCE_MARGIN_M, ROAD_DELTA_WARN_ABS_M, RoadBakeReport, RoadBakeWarning,
    affected_chunk_ids_for_network, ensure_chunk_road_deformation, fingerprint_road_network,
    influence_bounds_for_polyline,
    influence_bounds_for_road, rebake_road_deformation_for_chunks,
};
pub use persist::{
    ensure_road_deformation_store, load_road_deformation_bake, road_deformation_bake_dir,
    road_deformation_manifest_path, save_road_deformation_bake,
};
pub use refresh::{
    RoadTerrainRebuildQueue, apply_road_terrain_rebuilds, queue_road_terrain_rebuilds,
};
pub use store::{
    ROAD_DEFORMATION_BAKE_VERSION, RoadDeformationBakeDocument, RoadDeformationStore,
    affected_chunk_coords_for_bounds, chunk_coords_from_ids, chunk_ids_from_coords,
    sync_store_tiles_to_chunks, sync_store_tiles_to_resident_chunks,
};
pub use tile::RoadHeightDeltaTile;

#[cfg(test)]
mod tests;
