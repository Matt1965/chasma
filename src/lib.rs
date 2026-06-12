//! chasma: a Bevy large-world runtime and simulation foundation.
//!
//! The crate is organized as architectural layers (see ARCHITECTURE.md):
//! the application composition root (`app`), the authoritative World Data Layer
//! (`world`), and the Terrain Runtime Layer (`terrain`) which turns authoritative
//! chunk data into derived, disposable meshes (ADR-010, ROADMAP Phase 2).

pub mod app;
pub mod terrain;
pub mod world;
