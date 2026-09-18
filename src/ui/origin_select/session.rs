//! Client-local origin selection session (CG7).

use bevy::prelude::*;

use crate::world::OriginId;

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct OriginSelectSession {
    pub selected_index: usize,
    pub preview_yaw_radians: f32,
    pub preview_zoom: f32,
}

impl Default for OriginSelectSession {
    fn default() -> Self {
        Self {
            selected_index: 0,
            preview_yaw_radians: 0.0,
            preview_zoom: 1.0,
        }
    }
}

impl OriginSelectSession {
    pub fn selected_origin_id(
        &self,
        catalog: &crate::world::OriginCatalog,
    ) -> Option<OriginId> {
        catalog
            .get_index(self.selected_index)
            .map(|origin| origin.id.clone())
    }
}
