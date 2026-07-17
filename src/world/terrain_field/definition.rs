use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::category::TerrainFieldCategory;
use super::id::{TerrainFieldId, TerrainFieldSourceProfileId};
use super::overlay::TerrainFieldOverlayStyle;
use super::semantics::FieldValueSemantics;

/// Catalog definition for one continuous terrain field (ADR-101).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct TerrainFieldDefinition {
    pub id: TerrainFieldId,
    pub display_name: String,
    pub description: String,
    pub category: TerrainFieldCategory,
    pub value_semantics: FieldValueSemantics,
    pub overlay_style: TerrainFieldOverlayStyle,
    pub source_profile_id: Option<TerrainFieldSourceProfileId>,
    pub enabled: bool,
}

impl TerrainFieldDefinition {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        category: TerrainFieldCategory,
        value_semantics: FieldValueSemantics,
    ) -> Self {
        Self {
            id: TerrainFieldId::new(id),
            display_name: display_name.into(),
            description: String::new(),
            category,
            value_semantics,
            overlay_style: TerrainFieldOverlayStyle::default(),
            source_profile_id: None,
            enabled: true,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_overlay_style(mut self, overlay_style: TerrainFieldOverlayStyle) -> Self {
        self.overlay_style = overlay_style;
        self
    }

    pub fn with_source_profile_id(mut self, id: impl Into<String>) -> Self {
        self.source_profile_id = Some(TerrainFieldSourceProfileId::new(id));
        self
    }
}
