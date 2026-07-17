use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::world::asset_sizing::AssetSizingDefinition;
use crate::world::{BiomeId, DoodadDefinition, DoodadKind};
use crate::world::{BuildingCategoryDefinition, BuildingDefinition, FootprintSpec, FootprintType};

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
    #[serde(default)]
    pub asset_sizing: AssetSizingDefinition,
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
            render_key: definition.render_key.0.clone().unwrap_or_default(),
            allowed_biomes: definition
                .allowed_biomes
                .iter()
                .map(|biome| biome_to_string(*biome))
                .collect(),
            spawn_weight: definition.spawn_weight,
            random_rotation_y: definition.random_rotation_y,
            placement_tags: definition.placement_tags.clone(),
            asset_sizing: definition.asset_sizing.clone(),
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
    let text =
        ron::ser::to_string_pretty(&catalog, ron::ser::PrettyConfig::default()).map_err(|err| {
            DataImportError::Io {
                path: path.to_path_buf(),
                message: err.to_string(),
            }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildingCatalogRon {
    pub categories: Vec<BuildingCategoryRon>,
    pub definitions: Vec<BuildingDefinitionRon>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildingCategoryRon {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildingDefinitionRon {
    pub id: String,
    pub display_name: String,
    pub category_id: String,
    pub render_key: String,
    pub collision_render_key: String,
    pub preview_render_key: Option<String>,
    pub max_hp: u32,
    pub build_time_seconds: f32,
    pub footprint_type: String,
    pub footprint_width_meters: Option<f32>,
    pub footprint_depth_meters: Option<f32>,
    pub footprint_radius_meters: Option<f32>,
    pub construction_stages_ref: Option<String>,
    pub task_provider_id: Option<String>,
    pub animation_profile_id: Option<String>,
    pub interaction_profile_id: Option<String>,
    pub default_space_id: Option<String>,
    pub max_slope_degrees: f32,
    pub enabled: bool,
    #[serde(default)]
    pub asset_sizing: AssetSizingDefinition,
}

impl From<&BuildingCategoryDefinition> for BuildingCategoryRon {
    fn from(definition: &BuildingCategoryDefinition) -> Self {
        Self {
            id: definition.id.as_str().to_string(),
            display_name: definition.display_name.clone(),
            description: definition.description.clone(),
            enabled: definition.enabled,
        }
    }
}

impl From<&BuildingDefinition> for BuildingDefinitionRon {
    fn from(definition: &BuildingDefinition) -> Self {
        let (footprint_width_meters, footprint_depth_meters, footprint_radius_meters) =
            match &definition.footprint {
                FootprintSpec::Rectangle {
                    width_meters,
                    depth_meters,
                } => (Some(*width_meters), Some(*depth_meters), None),
                FootprintSpec::Circle { radius_meters } => (None, None, Some(*radius_meters)),
                FootprintSpec::MeshDerived => (None, None, None),
            };

        Self {
            id: definition.id.as_str().to_string(),
            display_name: definition.display_name.clone(),
            category_id: definition.category_id.as_str().to_string(),
            render_key: definition.render_key.0.clone().unwrap_or_default(),
            collision_render_key: definition
                .collision_render_key
                .0
                .clone()
                .unwrap_or_default(),
            preview_render_key: definition
                .preview_render_key
                .as_ref()
                .and_then(|key| key.0.clone()),
            max_hp: definition.max_hp,
            build_time_seconds: definition.build_time_seconds,
            footprint_type: definition.footprint_type.label().to_string(),
            footprint_width_meters,
            footprint_depth_meters,
            footprint_radius_meters,
            construction_stages_ref: definition.construction_stages_ref.clone(),
            task_provider_id: definition.task_provider_id.clone(),
            animation_profile_id: definition
                .animation_profile_id
                .as_ref()
                .map(|id| id.as_str().to_string()),
            interaction_profile_id: definition.interaction_profile_id.clone(),
            default_space_id: definition.default_space_id.clone(),
            max_slope_degrees: definition.max_slope_degrees,
            enabled: definition.enabled,
            asset_sizing: definition.asset_sizing.clone(),
        }
    }
}

pub fn export_buildings_to_ron(
    path: &Path,
    categories: &[BuildingCategoryDefinition],
    definitions: &[BuildingDefinition],
) -> Result<(), DataImportError> {
    let catalog = BuildingCatalogRon {
        categories: categories.iter().map(BuildingCategoryRon::from).collect(),
        definitions: definitions
            .iter()
            .map(BuildingDefinitionRon::from)
            .collect(),
    };
    let text =
        ron::ser::to_string_pretty(&catalog, ron::ser::PrettyConfig::default()).map_err(|err| {
            DataImportError::Io {
                path: path.to_path_buf(),
                message: err.to_string(),
            }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemCatalogRon {
    pub categories: Vec<ItemCategoryRon>,
    pub definitions: Vec<ItemDefinitionRon>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemCategoryRon {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub enabled: bool,
    pub sort_priority: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemDefinitionRon {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub category_id: String,
    pub grid_width: u8,
    pub grid_height: u8,
    pub stackable: bool,
    pub max_stack: u32,
    pub mass_grams_per_unit: u32,
    pub render_key: Option<String>,
    pub icon_key: Option<String>,
    pub base_value_gold: u32,
    pub tags: Vec<String>,
    pub unique_instance_required: bool,
    pub enabled: bool,
}

impl From<&crate::world::ItemCategoryDefinition> for ItemCategoryRon {
    fn from(definition: &crate::world::ItemCategoryDefinition) -> Self {
        Self {
            id: definition.id.as_str().to_string(),
            display_name: definition.display_name.clone(),
            description: definition.description.clone(),
            enabled: definition.enabled,
            sort_priority: definition.sort_priority,
        }
    }
}

impl From<&crate::world::ItemDefinition> for ItemDefinitionRon {
    fn from(definition: &crate::world::ItemDefinition) -> Self {
        Self {
            id: definition.id.as_str().to_string(),
            display_name: definition.display_name.clone(),
            description: definition.description.clone(),
            category_id: definition.category_id.as_str().to_string(),
            grid_width: definition.grid_width,
            grid_height: definition.grid_height,
            stackable: definition.stackable,
            max_stack: definition.max_stack,
            mass_grams_per_unit: definition.mass_grams_per_unit,
            render_key: definition.render_key.0.clone(),
            icon_key: definition.icon_key.0.clone(),
            base_value_gold: definition.base_value_gold,
            tags: definition.tags.clone(),
            unique_instance_required: definition.unique_instance_required,
            enabled: definition.enabled,
        }
    }
}

pub fn export_items_to_ron(
    path: &Path,
    categories: &[crate::world::ItemCategoryDefinition],
    definitions: &[crate::world::ItemDefinition],
) -> Result<(), DataImportError> {
    let catalog = ItemCatalogRon {
        categories: categories.iter().map(ItemCategoryRon::from).collect(),
        definitions: definitions.iter().map(ItemDefinitionRon::from).collect(),
    };
    let text =
        ron::ser::to_string_pretty(&catalog, ron::ser::PrettyConfig::default()).map_err(|err| {
            DataImportError::Io {
                path: path.to_path_buf(),
                message: err.to_string(),
            }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InventoryProfileCatalogRon {
    pub definitions: Vec<InventoryProfileRon>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InventoryProfileRon {
    pub id: String,
    pub display_name: String,
    pub grid_width: u8,
    pub grid_height: u8,
    pub reference_weight_grams: Option<u32>,
    pub global_stack_cap: Option<u32>,
    pub access_type: String,
    pub enabled: bool,
}

impl From<&crate::world::InventoryProfileDefinition> for InventoryProfileRon {
    fn from(definition: &crate::world::InventoryProfileDefinition) -> Self {
        Self {
            id: definition.id.as_str().to_string(),
            display_name: definition.display_name.clone(),
            grid_width: definition.grid_width,
            grid_height: definition.grid_height,
            reference_weight_grams: definition.reference_weight_grams,
            global_stack_cap: definition.global_stack_cap,
            access_type: format!("{:?}", definition.access_type),
            enabled: definition.enabled,
        }
    }
}

pub fn export_inventory_profiles_to_ron(
    path: &Path,
    definitions: &[crate::world::InventoryProfileDefinition],
) -> Result<(), DataImportError> {
    let catalog = InventoryProfileCatalogRon {
        definitions: definitions.iter().map(InventoryProfileRon::from).collect(),
    };
    let text =
        ron::ser::to_string_pretty(&catalog, ron::ser::PrettyConfig::default()).map_err(|err| {
            DataImportError::Io {
                path: path.to_path_buf(),
                message: err.to_string(),
            }
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

/// Export terrain field definitions to committed production RON (ADR-101 TF1).
pub fn export_terrain_fields_to_ron(
    path: &Path,
    definitions: &[crate::world::TerrainFieldDefinition],
) -> Result<(), DataImportError> {
    use crate::world::TerrainFieldCatalogRon;

    let catalog = TerrainFieldCatalogRon {
        definitions: definitions.to_vec(),
    };
    let text =
        ron::ser::to_string_pretty(&catalog, ron::ser::PrettyConfig::default()).map_err(|err| {
            DataImportError::Io {
                path: path.to_path_buf(),
                message: err.to_string(),
            }
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
    use crate::world::{
        BiomeId, DoodadDefinition, DoodadDefinitionId, DoodadKind, DoodadRenderKey,
    };

    #[test]
    fn item_catalog_ron_round_trip_fields() {
        use crate::world::{
            ItemCatalog, ItemCategoryCatalog, ItemCategoryDefinition, ItemCategoryId,
            ItemDefinition, ItemDefinitionId,
        };

        let categories = vec![ItemCategoryDefinition::new(
            ItemCategoryId::new("currency"),
            "Currency",
            "",
            true,
        )];
        let items = vec![ItemDefinition::new(
            ItemDefinitionId::new("gold"),
            "Gold",
            "",
            ItemCategoryId::new("currency"),
            1,
            1,
            true,
            999,
            1,
            1,
            true,
        )];
        let path = std::env::temp_dir().join(format!("chasma_item_catalog_{}", std::process::id()));
        export_items_to_ron(&path, &categories, &items).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("gold"));
        assert!(text.contains("currency"));
        let _ = fs::remove_file(path);
        let cat = ItemCategoryCatalog::from_definitions(categories).unwrap();
        let catalog = ItemCatalog::from_definitions(items, &cat).unwrap();
        assert_eq!(
            catalog
                .get(&ItemDefinitionId::new("gold"))
                .unwrap()
                .max_stack,
            999
        );
    }

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

        let path = std::env::temp_dir().join(format!("chasma_catalog_{}.ron", std::process::id()));
        export_doodads_to_ron(&path, &[definition]).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("tree_oak"));
        assert!(text.contains("Forest"));
        assert!(text.contains("random_rotation_y"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn exports_building_catalog_to_ron_text() {
        use crate::world::{
            BuildingCategoryDefinition, BuildingCategoryId, BuildingDefinition,
            BuildingDefinitionId, BuildingRenderKey, FootprintSpec,
        };

        let categories = vec![BuildingCategoryDefinition::new(
            BuildingCategoryId::new("residential"),
            "Residential",
            "Shelter",
            true,
        )];
        let definition = BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Hut",
            BuildingCategoryId::new("residential"),
            BuildingRenderKey::reserved("hut"),
            BuildingRenderKey::reserved("hut_collision"),
            100,
            30.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        );
        let path = std::env::temp_dir().join(format!(
            "chasma_building_catalog_{}.ron",
            std::process::id()
        ));
        export_buildings_to_ron(&path, &categories, &[definition]).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("hut"));
        assert!(text.contains("residential"));
        assert!(text.contains("footprint_type"));
        let _ = fs::remove_file(path);
    }
}
