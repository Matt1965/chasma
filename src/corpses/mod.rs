//! Corpse Runtime Layer — derived render entities for authoritative corpses.

mod components;
mod death_pose;
mod handoff;
mod plugin;
mod spawn;
mod sync;

#[cfg(test)]
mod presentation_tests;

pub use components::{CorpseDeathPosePending, CorpseRenderEntity, CorpseSceneRoot};
pub use handoff::handoff_unit_render_to_corpse;
pub use plugin::CorpseRuntimePlugin;
pub use spawn::spawn_corpse_render_entity;
pub use sync::{CorpseRenderIndex, CorpseRuntimeSystems, sync_corpse_render_entities};
