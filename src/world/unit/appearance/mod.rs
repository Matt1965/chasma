//! Unit appearance data and resolution (CG1).

mod catalog;
mod definition;
mod id;
mod morph;
#[cfg(test)]
mod morph_tests;
mod record;
mod resolve;

pub use catalog::{AppearanceProfileCatalog, AppearanceProfileCatalogError};
pub use definition::{
    AppearanceParameterDefinition, AppearanceProfile, BodyVariantDefinition, MorphMappingSide,
    MorphTargetMapping,
};
pub use morph::{
    CG2_MORPH_SEMANTIC_PARAMS, HUMAN_MORPH_SEMANTIC_PARAMS, HUMAN_MORPH_TARGET_NAMES,
    MorphResolveError,
    resolve_equipment_morph_weights, resolve_morph_weights, validate_profile_morph_mappings,
};
pub use id::{AppearanceParamId, AppearanceProfileId, BodyVariantId};
pub use record::UnitAppearance;
pub use resolve::{
    AppearanceError, definition_has_appearance_support, effective_render_key_for_appearance,
    effective_unit_render_key, effective_unit_render_key_str,
    resolve_canonical_default_appearance, validate_unit_appearance,
};
