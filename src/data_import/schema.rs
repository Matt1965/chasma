//! Excel column schema and conversion into [`DoodadDefinition`].

use crate::world::asset_sizing::{
    AssetSizingDefinition, DoodadCollisionShape, DoodadGroundingMode,
};
use crate::world::authoring_transform::AuthoringScale;
use crate::world::{
    BiomeId, DoodadDefinition, DoodadDefinitionId, DoodadKind, DoodadRenderKey,
    default_blocks_movement,
};

/// Required worksheet column headers (exact names; order irrelevant).
///
/// [`RANDOM_ROTATION_COLUMN_ALIASES`] satisfies the random-rotation requirement.
pub const REQUIRED_COLUMNS: &[&str] = &[
    "Name",
    "Description",
    "Category",
    "File Path",
    "Min Size",
    "Max Size",
    "Spawn Weight",
    "Enabled",
];

/// Accepted header names for the random-rotation column (any one is sufficient).
pub const RANDOM_ROTATION_COLUMN_ALIASES: &[&str] = &["Random Rotation", "Random Rotation (Y/N)"];

/// Optional column — when absent or blank, definitions allow all assigned biomes.
pub const BIOME_COLUMN: &str = "Biome";

/// Optional stable machine id column (preferred over slugified Name).
pub const DEFINITION_ID_COLUMN_ALIASES: &[&str] = &["Definition ID", "Doodad ID"];

/// Optional movement obstacle columns (ADR-031).
pub const BLOCKS_MOVEMENT_COLUMN: &str = "Blocks Movement";
pub const BLOCK_RADIUS_COLUMN: &str = "Block Radius";

/// Raw row parsed from the `Doodads` sheet before validation.
#[derive(Debug, Clone, PartialEq)]
pub struct DoodadImportRow {
    pub row_number: usize,
    /// Display name from the workbook `Name` column.
    pub name: String,
    /// Stable machine id when `Definition ID` column is present; otherwise derived.
    pub definition_id: String,
    pub description: String,
    pub category: String,
    pub biome: String,
    pub file_path: String,
    pub min_size: f32,
    pub max_size: f32,
    pub spawn_weight: f32,
    pub random_rotation: bool,
    pub enabled: bool,
    pub enabled_was_blank: bool,
    /// `None` when column absent or blank — use kind default.
    pub blocks_movement: Option<bool>,
    /// `None` when column absent or blank — defaults to placement radius.
    pub block_radius_meters: Option<f32>,
    pub asset_sizing: AssetSizingDefinition,
    pub default_instance_scale: AuthoringScale,
    pub allow_nonuniform_instance_scale: bool,
    pub collision_shape: DoodadCollisionShape,
    pub base_collision_radius_x_meters: Option<f32>,
    pub base_collision_radius_z_meters: Option<f32>,
    pub grounding_mode: DoodadGroundingMode,
}

pub fn parse_category(value: &str) -> Result<DoodadKind, String> {
    match value.trim() {
        "Tree" | "Flora" => Ok(DoodadKind::Tree),
        "Rock" | "Stone" => Ok(DoodadKind::Rock),
        "Bush" | "Shrub" => Ok(DoodadKind::Bush),
        "Ruin" => Ok(DoodadKind::Ruin),
        "ResourceNode" | "Resource" | "Resource Node" => Ok(DoodadKind::ResourceNode),
        other => Err(format!("unknown Category `{other}`")),
    }
}

pub fn parse_biome(value: &str) -> Result<BiomeId, String> {
    match value.trim() {
        "Desert" => Ok(BiomeId::Desert),
        "Forest" => Ok(BiomeId::Forest),
        "Marsh" => Ok(BiomeId::Marsh),
        "Plains" => Ok(BiomeId::Plains),
        other => Err(format!("unknown Biome `{other}`")),
    }
}

pub fn parse_bool_yn(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_uppercase().as_str() {
        "Y" | "YES" | "TRUE" | "1" => Ok(true),
        "N" | "NO" | "FALSE" | "0" => Ok(false),
        "" => Err("expected Y or N".to_string()),
        other => Err(format!("expected Y or N, got `{other}`")),
    }
}

pub fn parse_enabled_cell(value: &str) -> Result<(bool, bool), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok((true, true));
    }
    parse_bool_yn(trimmed).map(|enabled| (enabled, false))
}

/// Normalize a workbook file-path cell to canonical forward-slash form.
pub fn normalize_file_path(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    while normalized.starts_with('/') {
        normalized = normalized[1..].to_string();
    }
    normalized
}

/// Normalize an asset path cell into a [`DoodadRenderKey`] path segment (`tree/oak`).
pub fn normalize_file_path_to_render_key(path: &str) -> Result<String, String> {
    let mut key = normalize_file_path(path);
    if key.is_empty() {
        return Err("File Path must be non-empty".to_string());
    }
    for prefix in ["assets/doodads/", "doodads/"] {
        if let Some(rest) = key.strip_prefix(prefix) {
            key = rest.to_string();
        }
    }
    if let Some(stripped) = key.strip_suffix(".glb") {
        key = stripped.to_string();
    }
    if key.is_empty() {
        return Err("File Path must resolve to a render key".to_string());
    }
    Ok(key)
}

/// Resolve legacy folder-style design-sheet paths to shipped glTF stems.
pub fn canonical_doodad_render_key(key: String) -> String {
    match key.as_str() {
        // Chasma Design uses `\doodads\tree` for the oak test mesh at `tree/oak.glb`.
        "tree" => "tree/oak".to_string(),
        _ => key,
    }
}

/// Normalize a workbook display name into a stable machine id (REVIEW-B5).
pub fn normalize_doodad_definition_id(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Name must be non-empty".to_string());
    }
    if is_machine_definition_id(trimmed) {
        return Ok(trimmed.to_string());
    }
    let slug = slugify_definition_id(trimmed);
    if slug.is_empty() {
        return Err("Name must contain at least one alphanumeric character".to_string());
    }
    Ok(slug)
}

fn is_machine_definition_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn slugify_definition_id(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_sep = false;
    for c in value.trim().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            last_was_sep = false;
        } else if c.is_whitespace() || c == '-' || c == '_' {
            if !slug.is_empty() && !last_was_sep {
                slug.push('_');
                last_was_sep = true;
            }
        }
    }
    while slug.ends_with('_') {
        slug.pop();
    }
    slug
}

fn kind_defaults(kind: DoodadKind) -> (f32, Option<f32>) {
    match kind {
        DoodadKind::Tree => (4.0, Some(25.0)),
        DoodadKind::Rock => (3.0, Some(40.0)),
        DoodadKind::Bush => (1.5, Some(30.0)),
        DoodadKind::Ruin => (8.0, Some(15.0)),
        DoodadKind::ResourceNode => (3.0, Some(40.0)),
    }
}

fn allowed_biomes_for_row(biome: &str) -> Result<Vec<BiomeId>, String> {
    if biome.trim().is_empty() {
        return Ok(BiomeId::all_assigned().to_vec());
    }
    Ok(vec![parse_biome(biome)?])
}

impl DoodadImportRow {
    pub fn to_definition(&self) -> Result<DoodadDefinition, String> {
        let kind = parse_category(&self.category)?;
        let render_key =
            canonical_doodad_render_key(normalize_file_path_to_render_key(&self.file_path)?);
        let (placement_radius, max_slope) = kind_defaults(kind);
        let blocks_movement = self
            .blocks_movement
            .unwrap_or_else(|| default_blocks_movement(kind));
        let block_radius_meters = self.block_radius_meters.unwrap_or(placement_radius);
        let base_rx = self
            .base_collision_radius_x_meters
            .unwrap_or(block_radius_meters);
        let base_rz = self
            .base_collision_radius_z_meters
            .unwrap_or(block_radius_meters);

        let display_name = if self.description.trim().is_empty() {
            self.name.trim()
        } else {
            self.description.trim()
        };

        let mut definition = DoodadDefinition::new(
            DoodadDefinitionId::new(self.definition_id.trim()),
            kind,
            display_name,
            placement_radius,
            self.min_size,
            self.max_size,
            None,
            None,
            max_slope,
            self.enabled,
            DoodadRenderKey::reserved(render_key),
        )
        .with_allowed_biomes(allowed_biomes_for_row(&self.biome)?)
        .with_spawn_weight(self.spawn_weight)
        .with_random_rotation_y(self.random_rotation)
        .with_blocks_movement(blocks_movement)
        .with_block_radius_meters(block_radius_meters);
        definition.asset_sizing = self.asset_sizing.clone();
        definition.default_instance_scale = self.default_instance_scale;
        definition.allow_nonuniform_instance_scale = self.allow_nonuniform_instance_scale;
        definition.collision_shape = self.collision_shape;
        definition.base_collision_radius_x_meters = base_rx;
        definition.base_collision_radius_z_meters = base_rz;
        definition.grounding_mode = self.grounding_mode;
        Ok(definition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> DoodadImportRow {
        DoodadImportRow {
            row_number: 2,
            name: "tree_oak".to_string(),
            definition_id: "tree_oak".to_string(),
            description: "Oak Tree".to_string(),
            category: "Tree".to_string(),
            biome: "Forest".to_string(),
            file_path: "tree/oak.glb".to_string(),
            min_size: 0.85,
            max_size: 1.15,
            spawn_weight: 8.0,
            random_rotation: true,
            enabled: true,
            enabled_was_blank: false,
            blocks_movement: None,
            block_radius_meters: None,
            asset_sizing: AssetSizingDefinition::default(),
            default_instance_scale: AuthoringScale::uniform_one(),
            allow_nonuniform_instance_scale: true,
            collision_shape: DoodadCollisionShape::None,
            base_collision_radius_x_meters: None,
            base_collision_radius_z_meters: None,
            grounding_mode: DoodadGroundingMode::TerrainGrounded,
        }
    }

    fn basic_tree_row() -> DoodadImportRow {
        DoodadImportRow {
            row_number: 3,
            name: "Basic Tree".to_string(),
            definition_id: "basic_tree".to_string(),
            description: "Basic Tree".to_string(),
            category: "Flora".to_string(),
            biome: String::new(),
            file_path: r"\doodads\tree".to_string(),
            min_size: 0.5,
            max_size: 1.5,
            spawn_weight: 10.0,
            random_rotation: true,
            enabled: true,
            enabled_was_blank: false,
            blocks_movement: None,
            block_radius_meters: None,
            asset_sizing: AssetSizingDefinition::default(),
            default_instance_scale: AuthoringScale::uniform_one(),
            allow_nonuniform_instance_scale: true,
            collision_shape: DoodadCollisionShape::None,
            base_collision_radius_x_meters: None,
            base_collision_radius_z_meters: None,
            grounding_mode: DoodadGroundingMode::TerrainGrounded,
        }
    }

    #[test]
    fn category_parsing_includes_flora_alias() {
        assert_eq!(parse_category("Tree").unwrap(), DoodadKind::Tree);
        assert_eq!(parse_category("Flora").unwrap(), DoodadKind::Tree);
        assert_eq!(
            parse_category("Resource").unwrap(),
            DoodadKind::ResourceNode
        );
        assert!(parse_category("Unknown").is_err());
    }

    #[test]
    fn random_rotation_parsing_variants() {
        for value in ["Y", "Yes", "TRUE", "1"] {
            assert!(parse_bool_yn(value).unwrap());
        }
        for value in ["N", "No", "false", "0"] {
            assert!(!parse_bool_yn(value).unwrap());
        }
    }

    #[test]
    fn legacy_tree_folder_path_maps_to_oak_mesh() {
        assert_eq!(
            canonical_doodad_render_key(
                normalize_file_path_to_render_key(r"\doodads\tree").unwrap()
            ),
            "tree/oak"
        );
    }

    #[test]
    fn file_path_normalization() {
        assert_eq!(
            normalize_file_path(r"\doodads\tree\oak.glb"),
            "doodads/tree/oak.glb"
        );
        assert_eq!(
            normalize_file_path_to_render_key(r"\doodads\tree").unwrap(),
            "tree"
        );
        assert_eq!(
            normalize_file_path_to_render_key(r"\doodads\tree\oak.glb").unwrap(),
            "tree/oak"
        );
        assert!(normalize_file_path_to_render_key("").is_err());
    }

    #[test]
    fn enabled_blank_defaults_true() {
        let (enabled, blank) = parse_enabled_cell("").unwrap();
        assert!(enabled);
        assert!(blank);
    }

    #[test]
    fn converts_row_to_definition() {
        let def = sample_row().to_definition().unwrap();
        assert_eq!(def.id.as_str(), "tree_oak");
        assert_eq!(def.kind, DoodadKind::Tree);
        assert_eq!(def.spawn_weight, 8.0);
        assert_eq!(def.allowed_biomes, vec![BiomeId::Forest]);
        assert!(def.random_rotation_y);
        assert!((def.min_scale - 0.85).abs() < f32::EPSILON);
        assert!((def.max_scale - 1.15).abs() < f32::EPSILON);
        assert!(def.blocks_movement);
        assert_eq!(def.block_radius_meters, def.placement_radius_meters);
    }

    #[test]
    fn blocks_movement_column_overrides_kind_default() {
        let mut row = sample_row();
        row.category = "Bush".to_string();
        row.blocks_movement = Some(true);
        let def = row.to_definition().unwrap();
        assert!(def.blocks_movement);
    }

    #[test]
    fn block_radius_column_overrides_placement_default() {
        let mut row = sample_row();
        row.block_radius_meters = Some(9.5);
        let def = row.to_definition().unwrap();
        assert_eq!(def.block_radius_meters, 9.5);
        assert_eq!(def.placement_radius_meters, 4.0);
    }

    #[test]
    fn basic_tree_excel_row_maps_to_catalog_fields() {
        let def = basic_tree_row().to_definition().unwrap();
        assert_eq!(def.id.as_str(), "basic_tree");
        assert_eq!(def.display_name, "Basic Tree");
        assert_eq!(def.kind, DoodadKind::Tree);
        assert_eq!(def.render_key.0.as_deref(), Some("tree/oak"));
        assert!((def.spawn_weight - 10.0).abs() < f32::EPSILON);
        assert!(def.random_rotation_y);
        assert!((def.min_scale - 0.5).abs() < f32::EPSILON);
        assert!((def.max_scale - 1.5).abs() < f32::EPSILON);
        assert_eq!(def.allowed_biomes.len(), BiomeId::all_assigned().len());
    }

    #[test]
    fn normalize_slugifies_display_names() {
        assert_eq!(
            normalize_doodad_definition_id("Basic Tree").unwrap(),
            "basic_tree"
        );
        assert_eq!(
            normalize_doodad_definition_id("tree_oak").unwrap(),
            "tree_oak"
        );
        assert_eq!(
            normalize_doodad_definition_id("Basic-Tree").unwrap(),
            "basic_tree"
        );
    }

    #[test]
    fn fixed_scale_when_min_equals_max() {
        let mut row = sample_row();
        row.min_size = 1.2;
        row.max_size = 1.2;
        let def = row.to_definition().unwrap();
        assert!((def.min_scale - def.max_scale).abs() < f32::EPSILON);
    }
}
