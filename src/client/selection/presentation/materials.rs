//! Shared selection presentation materials.

use bevy::prelude::*;

use super::geometry::{SELECTION_GREEN, SELECTION_GREEN_PRIMARY};

/// Cached unlit selection materials (normal + primary unit).
#[derive(Resource, Default)]
pub struct SelectionPresentationMaterials {
    outline: Option<Handle<StandardMaterial>>,
    primary_outline: Option<Handle<StandardMaterial>>,
}

impl SelectionPresentationMaterials {
    pub fn outline(
        &mut self,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.outline
            .get_or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: SELECTION_GREEN,
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    cull_mode: None,
                    ..default()
                })
            })
            .clone()
    }

    pub fn primary_outline(
        &mut self,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.primary_outline
            .get_or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: SELECTION_GREEN_PRIMARY,
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    cull_mode: None,
                    ..default()
                })
            })
            .clone()
    }
}
