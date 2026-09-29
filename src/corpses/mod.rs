//! Corpse Runtime Layer — derived render entities for authoritative corpses.

mod components;
mod death_pose;
mod handoff;
mod ownership;
mod plugin;
mod spawn;
mod sync;

#[cfg(test)]
mod presentation_tests;

pub use components::{
    CorpseDeathPosePending, CorpsePresentationClaim, CorpseRenderEntity, CorpseSceneRoot,
};
pub use handoff::handoff_unit_render_to_corpse;
pub use ownership::{
    claim_corpse_presentation_for_death, corpse_origin_has_pending_death_root,
    corpse_presentation_entity_count, release_stale_corpse_presentation_owners,
    should_spawn_corpse_presentation,
};
pub use plugin::CorpseRuntimePlugin;
pub use spawn::spawn_corpse_render_entity;
pub use sync::{CorpseRenderIndex, CorpseRuntimeSystems};
