//! World navigation services (ADR-032 U7).

mod astar;
mod grid;
mod path;
mod query;
mod simplify;

pub use grid::{GridCoord, NEIGHBOR_OFFSETS, NavigationConfig};
pub use path::{NavigationPath, xz_distance};
pub use query::{NavigationError, find_path};
