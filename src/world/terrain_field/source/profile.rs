//! Terrain field source profile definition (ADR-102).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::super::id::{TerrainFieldId, TerrainFieldSourceProfileId};
use super::super::source_error::TerrainFieldSourceError;
use super::generator_config::GeneratedTerrainFieldSource;
use super::import_config::ImportedTerrainFieldSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerrainFieldSourceKind {
    ImportedMask,
    Generated,
    /// Reserved for TF4+ combined sources.
    Combined,
}

/// Catalog entry describing how to produce one field's base tiles (ADR-102).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainFieldSourceProfileDefinition {
    pub id: TerrainFieldSourceProfileId,
    pub display_name: String,
    pub source_kind: TerrainFieldSourceKind,
    pub output_field_id: TerrainFieldId,
    pub enabled: bool,
    pub profile_revision: String,
    pub imported: Option<ImportedTerrainFieldSource>,
    pub generated: Option<GeneratedTerrainFieldSource>,
}

impl TerrainFieldSourceProfileDefinition {
    pub fn generated(
        id: impl Into<String>,
        display_name: impl Into<String>,
        field_id: impl Into<String>,
        generated: GeneratedTerrainFieldSource,
    ) -> Self {
        Self {
            id: TerrainFieldSourceProfileId::new(id),
            display_name: display_name.into(),
            source_kind: TerrainFieldSourceKind::Generated,
            output_field_id: TerrainFieldId::new(field_id),
            enabled: true,
            profile_revision: "1".to_string(),
            imported: None,
            generated: Some(generated),
        }
    }

    pub fn imported(
        id: impl Into<String>,
        display_name: impl Into<String>,
        field_id: impl Into<String>,
        imported: ImportedTerrainFieldSource,
    ) -> Self {
        Self {
            id: TerrainFieldSourceProfileId::new(id),
            display_name: display_name.into(),
            source_kind: TerrainFieldSourceKind::ImportedMask,
            output_field_id: TerrainFieldId::new(field_id),
            enabled: true,
            profile_revision: "1".to_string(),
            imported: Some(imported),
            generated: None,
        }
    }

    pub fn validate(&self) -> Result<(), TerrainFieldSourceError> {
        match self.source_kind {
            TerrainFieldSourceKind::ImportedMask => {
                let imported = self.imported.as_ref().ok_or_else(|| {
                    TerrainFieldSourceError::InvalidSourceConfiguration(
                        "imported profile missing imported config".to_string(),
                    )
                })?;
                imported.validate()?;
            }
            TerrainFieldSourceKind::Generated => {
                let generated = self.generated.as_ref().ok_or_else(|| {
                    TerrainFieldSourceError::InvalidSourceConfiguration(
                        "generated profile missing generated config".to_string(),
                    )
                })?;
                if generated.generator_version
                    != super::generator_config::TERRAIN_FIELD_GENERATOR_VERSION
                {
                    return Err(TerrainFieldSourceError::GeneratorVersionUnsupported {
                        found: generated.generator_version,
                        expected: super::generator_config::TERRAIN_FIELD_GENERATOR_VERSION,
                    });
                }
            }
            TerrainFieldSourceKind::Combined => {
                return Err(TerrainFieldSourceError::UnsupportedSourceKind(
                    "Combined".to_string(),
                ));
            }
        }
        Ok(())
    }
}
