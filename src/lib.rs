//! chasma: a Bevy large-world runtime and simulation foundation.
//!
//! The crate is organized as architectural layers (see ARCHITECTURE.md):
//! the application composition root (`app`), the authoritative World Data Layer
//! (`world`), the Terrain Runtime Layer (`terrain`), and the client-local Camera
//! layer (`camera`) (ADR-010, ADR-014, ROADMAP Phase 2).

pub mod app;
pub mod camera;
pub mod client;
#[cfg(feature = "dev")]
pub mod dev;
pub mod data_import;
pub mod debug;
pub mod doodads;
pub mod environment;
pub mod logging;
pub mod terrain;
pub mod player;
pub mod simulation;
pub mod ui;
pub mod units;
pub mod view;
pub mod world;

#[cfg(test)]
mod review;
