//! World navigation services (ADR-032 U7).

mod astar;
mod grid;
mod path;
mod query;

pub use grid::{GridCoord, NavigationConfig, NEIGHBOR_OFFSETS};
pub use path::NavigationPath;
pub use query::{find_path, NavigationError};
