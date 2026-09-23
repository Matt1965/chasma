//! Seek final death pose for reconstructed corpse render roots.

use bevy::prelude::*;

use crate::units::{
    AnimationPlaybackPending, DeathPresentation, UnitAnimationLayering, UnitAnimationSettings,
};
use crate::world::AnimationProfileCatalog;

use super::components::CorpseDeathPosePending;

/// Convert reconstructed corpse markers into death presentation playback.
pub fn begin_corpse_death_poses(
    mut commands: Commands,
    settings: Res<UnitAnimationSettings>,
    profiles: Res<AnimationProfileCatalog>,
    pending: Query<(Entity, &CorpseDeathPosePending), Without<DeathPresentation>>,
) {
    if !settings.enabled {
        return;
    }
    for (entity, marker) in &pending {
        let Some(profile) = profiles.get(&marker.profile_id) else {
            commands.entity(entity).remove::<CorpseDeathPosePending>();
            continue;
        };
        let has_death_clip = profile.resolve_death_clip_name().is_some();
        commands.entity(entity).insert((
            DeathPresentation {
                origin_unit_id: None,
                definition_id: marker.definition_id.clone(),
                profile_id: marker.profile_id.clone(),
                remaining_seconds: f32::MAX,
                freeze_pose: marker.freeze_pose || !has_death_clip,
            },
            UnitAnimationLayering::full_body_exclusive(),
            AnimationPlaybackPending,
        ));
        commands.entity(entity).remove::<CorpseDeathPosePending>();
    }
}
