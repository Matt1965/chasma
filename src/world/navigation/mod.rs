//! World navigation services (ADR-032 U7).

mod astar;
mod cross_space;
mod grid;
mod path;
mod query;
mod simplify;
mod waypoint;

pub use cross_space::{find_path_in_spaces, is_position_walkable_in_space};
pub use grid::{
    GridCoord, NEIGHBOR_OFFSETS, NavigationAgent, NavigationConfig, grid_cell_center_global,
    grid_cell_world_position, grid_coord_at_global_xz, grid_coord_at_position, is_cell_walkable,
    is_position_walkable,
};
pub use path::{NavigationPath, xz_distance};
pub use query::{NavigationError, find_path, find_path_with_spaces};
pub use waypoint::NavigationWaypoint;
