//! World navigation services (ADR-032 U7).

mod astar;
mod cross_space;
mod grid;
mod interior_clearance;
mod path;
mod query;
mod simplify;
mod waypoint;

#[cfg(test)]
mod interior_clearance_tests;
#[cfg(test)]
mod interior_path_tests;

pub use cross_space::find_path_in_spaces;
pub use grid::{
    GridCoord, NEIGHBOR_OFFSETS, NavigationAgent, NavigationConfig,
    cell_walkability_sample_globals, grid_cell_center_global, grid_cell_world_position,
    grid_coord_at_global_xz, grid_coord_at_position, is_cell_walkable, is_cell_walkable_in_space,
    is_position_walkable, is_position_walkable_in_space, resolve_path_endpoint_cell,
};
pub use interior_clearance::{
    InteriorCellFailureReason, InteriorCellProbe, InteriorRegionClearanceReport,
    inset_polygon_toward_centroid, measure_interior_region_clearance, min_edge_clearance_meters,
    polygon_axis_span, signed_distance_to_polygon_edges,
};
pub use path::{NavigationPath, xz_distance};
pub use query::{NavigationError, find_path, find_path_with_spaces};
pub use simplify::{is_segment_walkable_in_space, simplify_navigation_path_in_space};
pub use waypoint::NavigationWaypoint;
