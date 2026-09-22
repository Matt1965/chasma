use crate::world::relationship::SpeciesId;
use crate::world::{
    AppearanceParameterDefinition, AppearanceProfile, AppearanceProfileId, BodyVariantDefinition,
    BodyVariantId, MorphMappingSide, MorphTargetMapping, UnitRenderKey,
};

pub const PROFILE_REQUIRED_COLUMNS: &[&str] = &[
    "Profile ID",
    "Species ID",
    "Enabled",
    "Schema Version",
    "Height Min",
    "Height Max",
    "Height Default",
];

pub const VARIANT_REQUIRED_COLUMNS: &[&str] = &[
    "Profile ID",
    "Variant ID",
    "Render Key",
    "Display Name",
    "Enabled",
];

pub const MORPH_MAPPING_REQUIRED_COLUMNS: &[&str] = &[
    "Profile ID",
    "Variant ID",
    "Param ID",
    "Target Name",
    "Side",
    "Multiplier",
    "Enabled",
];

pub const PARAMETER_REQUIRED_COLUMNS: &[&str] = &[
    "Profile ID",
    "Param ID",
    "Display Name",
    "Category",
    "Min",
    "Max",
    "Default",
    "Display Order",
    "Enabled",
];

#[derive(Debug, Clone, PartialEq)]
pub struct AppearanceProfileImportRow {
    pub row_number: usize,
    pub profile_id: String,
    pub species_id: String,
    pub enabled: bool,
    pub schema_version: u32,
    pub height_min: f32,
    pub height_max: f32,
    pub height_default: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppearanceBodyVariantImportRow {
    pub row_number: usize,
    pub profile_id: String,
    pub variant_id: String,
    pub render_key: String,
    pub display_name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppearanceMorphMappingImportRow {
    pub row_number: usize,
    pub profile_id: String,
    pub variant_id: String,
    pub param_id: String,
    pub target_name: String,
    pub side: MorphMappingSide,
    pub multiplier: f32,
    pub enabled: bool,
}

impl AppearanceMorphMappingImportRow {
    pub fn validate(&self) -> Result<(), String> {
        if self.variant_id.trim().is_empty() {
            return Err("Variant ID must be non-empty".to_string());
        }
        if self.param_id.trim().is_empty() {
            return Err("Param ID must be non-empty".to_string());
        }
        if self.target_name.trim().is_empty() {
            return Err("Target Name must be non-empty".to_string());
        }
        if !self.multiplier.is_finite() || self.multiplier < 0.0 {
            return Err("Multiplier must be a finite non-negative number".to_string());
        }
        Ok(())
    }

    pub fn to_definition(&self) -> MorphTargetMapping {
        MorphTargetMapping {
            variant_id: BodyVariantId::new(self.variant_id.trim()),
            param_id: crate::world::AppearanceParamId::new(self.param_id.trim()),
            technical_target: self.target_name.trim().to_string(),
            side: self.side,
            multiplier: self.multiplier,
            enabled: self.enabled,
        }
    }
}

pub fn parse_morph_mapping_side(raw: &str) -> Result<MorphMappingSide, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "above default" | "above_default" | "above" => Ok(MorphMappingSide::AboveDefault),
        "below default" | "below_default" | "below" => Ok(MorphMappingSide::BelowDefault),
        other => Err(format!("Side must be `Above Default` or `Below Default` (got `{other}`)")),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppearanceParameterImportRow {
    pub row_number: usize,
    pub profile_id: String,
    pub param_id: String,
    pub display_name: String,
    pub category: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub display_order: u32,
    pub enabled: bool,
}

impl AppearanceParameterImportRow {
    pub fn validate(&self) -> Result<(), String> {
        if self.param_id.trim().is_empty() {
            return Err("Param ID must be non-empty".to_string());
        }
        if !self.min.is_finite() || !self.max.is_finite() || !self.default.is_finite() {
            return Err("Min/Max/Default must be finite".to_string());
        }
        if self.min > self.max {
            return Err(format!(
                "Min ({}) must be <= Max ({})",
                self.min,
                self.max
            ));
        }
        if self.default < self.min || self.default > self.max {
            return Err(format!(
                "Default ({}) must be within [{}, {}]",
                self.default,
                self.min,
                self.max
            ));
        }
        Ok(())
    }

    pub fn to_definition(&self) -> AppearanceParameterDefinition {
        AppearanceParameterDefinition {
            id: crate::world::AppearanceParamId::new(self.param_id.trim()),
            display_name: self.display_name.trim().to_string(),
            category: self.category.trim().to_string(),
            min: self.min,
            max: self.max,
            default: self.default,
            display_order: self.display_order,
            enabled: self.enabled,
        }
    }
}

impl AppearanceBodyVariantImportRow {
    pub fn validate(&self) -> Result<(), String> {
        if self.variant_id.trim().is_empty() {
            return Err("Variant ID must be non-empty".to_string());
        }
        if self.render_key.trim().is_empty() {
            return Err("Render Key must be non-empty".to_string());
        }
        Ok(())
    }

    pub fn to_definition(&self) -> Result<BodyVariantDefinition, String> {
        self.validate()?;
        Ok(BodyVariantDefinition {
            id: BodyVariantId::new(self.variant_id.trim()),
            display_name: self.display_name.trim().to_string(),
            render_key: UnitRenderKey::reserved(self.render_key.trim()),
            enabled: self.enabled,
        })
    }
}

impl AppearanceProfileImportRow {
    pub fn validate(&self) -> Result<(), String> {
        if self.profile_id.trim().is_empty() {
            return Err("Profile ID must be non-empty".to_string());
        }
        if self.species_id.trim().is_empty() {
            return Err("Species ID must be non-empty".to_string());
        }
        if !self.height_min.is_finite()
            || !self.height_max.is_finite()
            || !self.height_default.is_finite()
        {
            return Err("Height Min/Max/Default must be finite".to_string());
        }
        if self.height_min > self.height_max {
            return Err("Height Min must be <= Height Max".to_string());
        }
        if self.height_default < self.height_min || self.height_default > self.height_max {
            return Err("Height Default must be within Height Min/Max".to_string());
        }
        Ok(())
    }
}

pub fn assemble_profiles(
    profile_rows: Vec<AppearanceProfileImportRow>,
    variant_rows: Vec<AppearanceBodyVariantImportRow>,
    parameter_rows: Vec<AppearanceParameterImportRow>,
    mapping_rows: Vec<AppearanceMorphMappingImportRow>,
) -> Result<Vec<AppearanceProfile>, String> {
    use std::collections::{BTreeMap, HashSet};

    let mut profiles: BTreeMap<String, AppearanceProfile> = BTreeMap::new();

    for row in profile_rows {
        row.validate()
            .map_err(|message| format!("profile row {}: {message}", row.row_number))?;
        let profile_id = row.profile_id.trim();
        if profiles.contains_key(profile_id) {
            return Err(format!("duplicate Profile ID `{profile_id}`"));
        }
        profiles.insert(
            profile_id.to_string(),
            AppearanceProfile {
                id: AppearanceProfileId::new(profile_id),
                species_id: SpeciesId::new(row.species_id.trim()),
                schema_version: row.schema_version,
                height_scale_min: row.height_min,
                height_scale_max: row.height_max,
                height_scale_default: row.height_default,
                body_variants: Vec::new(),
                parameters: Vec::new(),
                morph_mappings: Vec::new(),
                enabled: row.enabled,
            },
        );
    }

    let mut seen_variants: HashSet<(String, String)> = HashSet::new();
    for row in variant_rows {
        let profile_id = row.profile_id.trim();
        if !profiles.contains_key(profile_id) {
            return Err(format!(
                "body variant row {}: unknown Profile ID `{}`",
                row.row_number,
                profile_id
            ));
        }
        let variant_id = row.variant_id.trim();
        let key = (profile_id.to_string(), variant_id.to_string());
        if !seen_variants.insert(key) {
            return Err(format!(
                "duplicate body variant `{variant_id}` in profile `{profile_id}`"
            ));
        }
        let definition = row
            .to_definition()
            .map_err(|message| format!("body variant row {}: {message}", row.row_number))?;
        profiles
            .get_mut(profile_id)
            .expect("profile exists")
            .body_variants
            .push(definition);
    }

    let mut seen_params: HashSet<(String, String)> = HashSet::new();
    for row in parameter_rows {
        row.validate()
            .map_err(|message| format!("parameter row {}: {message}", row.row_number))?;
        let profile_id = row.profile_id.trim();
        if !profiles.contains_key(profile_id) {
            return Err(format!(
                "parameter row {}: unknown Profile ID `{}`",
                row.row_number,
                profile_id
            ));
        }
        let param_id = row.param_id.trim();
        let key = (profile_id.to_string(), param_id.to_string());
        if !seen_params.insert(key) {
            return Err(format!(
                "duplicate parameter `{param_id}` in profile `{profile_id}`"
            ));
        }
        profiles
            .get_mut(profile_id)
            .expect("profile exists")
            .parameters
            .push(row.to_definition());
    }

    let mut seen_mappings: HashSet<(String, String, String)> = HashSet::new();
    for row in mapping_rows {
        row.validate()
            .map_err(|message| format!("morph mapping row {}: {message}", row.row_number))?;
        let profile_id = row.profile_id.trim();
        if !profiles.contains_key(profile_id) {
            return Err(format!(
                "morph mapping row {}: unknown Profile ID `{}`",
                row.row_number,
                profile_id
            ));
        }
        let key = (
            profile_id.to_string(),
            row.variant_id.trim().to_string(),
            row.target_name.trim().to_string(),
        );
        if !seen_mappings.insert(key) {
            return Err(format!(
                "duplicate morph mapping target `{}` for variant `{}` in profile `{}`",
                row.target_name.trim(),
                row.variant_id.trim(),
                profile_id
            ));
        }
        profiles
            .get_mut(profile_id)
            .expect("profile exists")
            .morph_mappings
            .push(row.to_definition());
    }

    for profile in profiles.values_mut() {
        crate::world::validate_profile_morph_mappings(profile)
            .map_err(|message| format!("profile `{}`: {message}", profile.id.as_str()))?;
    }

    for profile in profiles.values() {
        if profile.body_variants.is_empty() {
            return Err(format!(
                "appearance profile `{}` has no body variants",
                profile.id.as_str()
            ));
        }
        if profile.parameters.is_empty() {
            return Err(format!(
                "appearance profile `{}` has no parameters",
                profile.id.as_str()
            ));
        }
        let enabled_variants = profile
            .body_variants
            .iter()
            .filter(|variant| variant.enabled)
            .count();
        if enabled_variants == 0 {
            return Err(format!(
                "appearance profile `{}` has no enabled body variants",
                profile.id.as_str()
            ));
        }
        let enabled_params = profile
            .parameters
            .iter()
            .filter(|parameter| parameter.enabled)
            .count();
        if enabled_params == 0 {
            return Err(format!(
                "appearance profile `{}` has no enabled parameters",
                profile.id.as_str()
            ));
        }
    }

    Ok(profiles.into_values().collect())
}
