use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::ProjectileId;

/// Maps authoritative projectile ids to runtime render entities.
#[derive(Resource, Debug, Default)]
pub struct ProjectileRenderIndex(pub HashMap<ProjectileId, Entity>);
