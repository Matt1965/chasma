//! ECS markers for isolated unit portrait capture.

use bevy::prelude::*;

use crate::world::UnitId;

/// Root of the portrait capture stage (lights, backdrop, actor slot).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitPortraitStageRoot;

/// Dedicated camera that renders only [`PORTRAIT_RENDER_LAYER`] into a capture target.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitPortraitCamera;

/// glTF scene root for a portrait actor (not a gameplay [`UnitRenderEntity`]).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitPortraitSceneRoot;

/// One transient portrait actor spawned for an in-flight capture request.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct UnitPortraitActor {
    pub unit_id: UnitId,
    pub request_generation: u64,
}

/// Cached head-and-shoulders framing for a portrait actor.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct UnitPortraitFraming {
    pub focus: Vec3,
    pub head_height: f32,
}
