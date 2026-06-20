//! Excel column schema and conversion into [`DoodadDefinition`].

use crate::world::{BiomeId, DoodadDefinition, DoodadDefinitionId, DoodadKind, DoodadRenderKey};

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

/// Raw row parsed from the `Doodads` sheet before validation.
#[derive(Debug, Clone, PartialEq)]
pub struct DoodadImportRow {
    pub row_number: usize,
    pub name: String,
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
        let render_key = normalize_file_path_to_render_key(&self.file_path)?;
        let (placement_radius, max_slope) = kind_defaults(kind);

        Ok(
            DoodadDefinition::new(
                DoodadDefinitionId::new(self.name.trim()),
                kind,
                self.description.trim(),
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
            .with_random_rotation_y(self.random_rotation),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> DoodadImportRow {
        DoodadImportRow {
            row_number: 2,
            name: "tree_oak".to_string(),
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
        }
    }

    fn basic_tree_row() -> DoodadImportRow {
        DoodadImportRow {
            row_number: 3,
            name: "Basic Tree".to_string(),
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
        }
    }

    #[test]
    fn category_parsing_includes_flora_alias() {
        assert_eq!(parse_category("Tree").unwrap(), DoodadKind::Tree);
        assert_eq!(parse_category("Flora").unwrap(), DoodadKind::Tree);
        assert_eq!(parse_category("Resource").unwrap(), DoodadKind::ResourceNode);
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
    }

    #[test]
    fn basic_tree_excel_row_maps_to_catalog_fields() {
        let def = basic_tree_row().to_definition().unwrap();
        assert_eq!(def.id.as_str(), "Basic Tree");
        assert_eq!(def.kind, DoodadKind::Tree);
        assert_eq!(def.render_key.0.as_deref(), Some("tree"));
        assert!((def.spawn_weight - 10.0).abs() < f32::EPSILON);
        assert!(def.random_rotation_y);
        assert!((def.min_scale - 0.5).abs() < f32::EPSILON);
        assert!((def.max_scale - 1.5).abs() < f32::EPSILON);
        assert_eq!(def.allowed_biomes.len(), BiomeId::all_assigned().len());
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
