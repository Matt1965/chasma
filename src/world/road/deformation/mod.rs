//! Derived per-chunk road terrain deformation (effective height deltas).

mod bake;
mod persist;
mod refresh;
mod store;
mod tile;

pub use bake::{
    affected_chunk_ids_for_network, ensure_chunk_road_deformation, rebake_road_deformation_for_chunks,
};
pub use persist::{
    ensure_road_deformation_store, save_road_deformation_bake,
};
pub use refresh::{
    RoadTerrainRebuildQueue, apply_road_terrain_rebuilds, queue_road_terrain_rebuilds,
    reconcile_road_deformation_on_startup,
};
pub use store::{
    RoadDeformationStore,
    sync_store_tiles_to_chunks, sync_store_tiles_to_resident_chunks,
};
pub use tile::RoadHeightDeltaTile;

#[cfg(test)]
mod tests;
