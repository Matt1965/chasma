//! Procedural doodad generation — deterministic candidates only (ADR-018, Phase 3D).
//!
//! Answers "what doodads *would* exist in this chunk?" without mutating
//! [`crate::world::WorldData`] or spawning ECS entities.

mod candidate;
mod context;
mod generator;
mod rng;
mod settings;
mod weighted;

pub use candidate::DoodadSpawnCandidate;
pub use context::DoodadGenerationContext;
pub use generator::{generate_chunk_doodads, generate_chunk_doodads_with_settings};
pub use settings::DoodadGenerationSettings;
pub use weighted::format_candidate_summary;
