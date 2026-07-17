//! Shared authoritative transform contract (ADR-097 DT1).

use bevy::prelude::*;

use crate::world::WorldPosition;

use super::orientation::QuantizedOrientation;
use super::scale::AuthoringScale;

/// Authoritative placement transform — distinct from Bevy presentation [`bevy::prelude::Transform`].
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct AuthoringTransform {
    pub position: WorldPosition,
    pub orientation: QuantizedOrientation,
    pub scale: AuthoringScale,
}

impl AuthoringTransform {
    pub fn identity_at(position: WorldPosition) -> Self {
        Self {
            position,
            orientation: QuantizedOrientation::IDENTITY,
            scale: AuthoringScale::uniform_one(),
        }
    }
}
