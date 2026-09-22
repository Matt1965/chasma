use bevy::prelude::*;

use crate::world::CorpseId;

/// Marker on a derived corpse render root.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct CorpseRenderEntity {
    pub corpse_id: CorpseId,
}

/// glTF scene root for a corpse render entity.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct CorpseSceneRoot;

/// Reconstructed corpse awaiting final death-pose seek (scene load / sync spawn).
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct CorpseDeathPosePending {
    pub definition_id: crate::world::UnitDefinitionId,
    pub profile_id: crate::world::AnimationProfileId,
    pub freeze_pose: bool,
}
