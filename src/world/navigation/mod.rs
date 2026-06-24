//! World navigation services (ADR-032 U7).

mod astar;
mod grid;
mod path;
mod query;
mod simplify;

pub use grid::{GridCoord, NavigationConfig, NEIGHBOR_OFFSETS};
pub use path::{NavigationPath, xz_distance};
pub use query::{find_path, NavigationError};
