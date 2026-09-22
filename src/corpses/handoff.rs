//! Retarget live unit render roots into persistent corpse presentation.

use bevy::prelude::*;

use crate::units::{DeathPresentation, UnitRenderEntity, UnitRenderIndex, UnitSceneRoot};
use crate::world::{CorpseId, UnitId};

use super::components::{CorpsePresentationClaim, CorpseRenderEntity, CorpseSceneRoot};
use super::sync::CorpseRenderIndex;

/// Retarget an existing unit render entity into corpse presentation ownership.
pub fn handoff_unit_render_to_corpse(
    commands: &mut Commands,
    unit_index: &mut UnitRenderIndex,
    corpse_index: &mut CorpseRenderIndex,
    entity: Entity,
    unit_id: UnitId,
    corpse_id: CorpseId,
) {
    unit_index.0.remove(&unit_id);
    corpse_index.0.insert(corpse_id, entity);
    commands.entity(entity).insert(CorpseRenderEntity { corpse_id });
    commands
        .entity(entity)
        .remove::<UnitRenderEntity>()
        .remove::<UnitSceneRoot>()
        .remove::<DeathPresentation>()
        .remove::<CorpsePresentationClaim>()
        .insert(CorpseSceneRoot);
}
