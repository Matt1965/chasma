use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::world::{BiomeId, DoodadDefinition, DoodadKind};

use super::error::DataImportError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DoodadCatalogRon {
    pub definitions: Vec<DoodadDefinitionRon>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DoodadDefinitionRon {
    pub id: String,
    pub kind: String,
    pub display_name: String,
    pub placement_radius_meters: f32,
    pub min_scale: f32,
    pub max_scale: f32,
    pub max_slope_degrees: Option<f32>,
    pub enabled: bool,
    pub render_key: String,
    pub allowed_biomes: Vec<String>,
    pub spawn_weight: f32,
    pub random_rotation_y: bool,
    pub placement_tags: Vec<String>,
}

impl From<&DoodadDefinition> for DoodadDefinitionRon {
    fn from(definition: &DoodadDefinition) -> Self {
        Self {
            id: definition.id.as_str().to_string(),
            kind: kind_to_string(definition.kind),
            display_name: definition.display_name.clone(),
            placement_radius_meters: definition.placement_radius_meters,
            min_scale: definition.min_scale,
            max_scale: definition.max_scale,
            max_slope_degrees: definition.max_slope_degrees,
            enabled: definition.enabled,
            render_key: definition
                .render_key
                .0
                .clone()
                .unwrap_or_default(),
            allowed_biomes: definition
                .allowed_biomes
                .iter()
                .map(|biome| biome_to_string(*biome))
                .collect(),
            spawn_weight: definition.spawn_weight,
            random_rotation_y: definition.random_rotation_y,
            placement_tags: definition.placement_tags.clone(),
        }
    }
}

fn kind_to_string(kind: DoodadKind) -> String {
    match kind {
        DoodadKind::Tree => "Tree",
        DoodadKind::Rock => "Rock",
        DoodadKind::Bush => "Bush",
        DoodadKind::Ruin => "Ruin",
        DoodadKind::ResourceNode => "ResourceNode",
    }
    .to_string()
}

fn biome_to_string(biome: BiomeId) -> String {
    match biome {
        BiomeId::Desert => "Desert",
        BiomeId::Forest => "Forest",
        BiomeId::Marsh => "Marsh",
        BiomeId::Plains => "Plains",
        BiomeId::Unassigned => "Unassigned",
    }
    .to_string()
}

pub fn export_doodads_to_ron(
    path: &Path,
    definitions: &[DoodadDefinition],
) -> Result<(), DataImportError> {
    let catalog = DoodadCatalogRon {
        definitions: definitions.iter().map(DoodadDefinitionRon::from).collect(),
    };
    let text = ron::ser::to_string_pretty(&catalog, ron::ser::PrettyConfig::default())
        .map_err(|err| DataImportError::Io {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| DataImportError::Io {
            path: parent.to_path_buf(),
            message: err.to_string(),
        })?;
    }
    fs::write(path, text).map_err(|err| DataImportError::Io {
        path: path.to_path_buf(),
        message: err.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BiomeId, DoodadDefinition, DoodadDefinitionId, DoodadKind, DoodadRenderKey};

    #[test]
    fn exports_definitions_to_ron_text() {
        let definition = DoodadDefinition::new(
            DoodadDefinitionId::new("tree_oak"),
            DoodadKind::Tree,
            "Oak Tree",
            4.0,
            0.85,
            1.15,
            None,
            None,
            Some(25.0),
            true,
            DoodadRenderKey::reserved("tree/oak"),
        )
        .with_allowed_biomes(vec![BiomeId::Forest])
        .with_spawn_weight(8.0)
        .with_random_rotation_y(true);

        let path = std::env::temp_dir().join(format!(
            "chasma_catalog_{}.ron",
            std::process::id()
        ));
        export_doodads_to_ron(&path, &[definition]).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("tree_oak"));
        assert!(text.contains("Forest"));
        assert!(text.contains("random_rotation_y"));
        let _ = fs::remove_file(path);
    }
}
