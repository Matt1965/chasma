//! CG5 consumed morph param validation for equipment visual mappings.

use crate::world::equipment::{EquipmentPresentationMode, EquipmentVisualMapping};
use crate::world::{
    AppearanceProfile, AppearanceProfileCatalog, BodyVariantId, MorphTargetMapping,
};

pub fn validate_equipment_visual_mapping(
    mapping: &EquipmentVisualMapping,
    appearance_profiles: &AppearanceProfileCatalog,
    row_number: usize,
) -> Result<(), String> {
    if mapping.consumed_morph_params.is_empty() {
        return Ok(());
    }
    if mapping.mode != EquipmentPresentationMode::SkinnedOverlay {
        return Err(format!(
            "row {}: rigid attachment for item `{}` on `{}` must not declare consumed morph params",
            row_number,
            mapping.item_id.as_str(),
            mapping.unit_render_key
        ));
    }
    let profile = profile_for_unit_render_key(appearance_profiles, &mapping.unit_render_key)
        .ok_or_else(|| {
            format!(
                "row {}: no appearance profile owns unit render key `{}`",
                row_number,
                mapping.unit_render_key
            )
        })?;
    let variant_id = body_variant_for_render_key(profile, &mapping.unit_render_key).ok_or_else(
        || {
            format!(
                "row {}: no enabled body variant for unit render key `{}`",
                row_number,
                mapping.unit_render_key
            )
        },
    )?;
    let mappings = morph_mappings_for_variant(profile, &variant_id);
    for param in &mapping.consumed_morph_params {
        if profile.parameter(param).is_none() {
            return Err(format!(
                "row {}: unknown consumed morph param `{}` for profile `{}`",
                row_number,
                param.as_str(),
                profile.id.as_str()
            ));
        }
        let has_mapping = mappings.iter().any(|mapping| mapping.param_id == *param);
        if !has_mapping {
            return Err(format!(
                "row {}: consumed morph param `{}` has no morph mapping for variant `{}` in profile `{}`",
                row_number,
                param.as_str(),
                variant_id.as_str(),
                profile.id.as_str()
            ));
        }
    }
    Ok(())
}

fn profile_for_unit_render_key<'a>(
    appearance_profiles: &'a AppearanceProfileCatalog,
    unit_render_key: &str,
) -> Option<&'a AppearanceProfile> {
    appearance_profiles
        .definitions()
        .iter()
        .find(|profile| body_variant_for_render_key(profile, unit_render_key).is_some())
}

fn body_variant_for_render_key(
    profile: &AppearanceProfile,
    unit_render_key: &str,
) -> Option<BodyVariantId> {
    profile
        .body_variants
        .iter()
        .find(|variant| {
            variant.enabled && variant.render_key.0.as_deref() == Some(unit_render_key)
        })
        .map(|variant| variant.id.clone())
}

fn morph_mappings_for_variant(
    profile: &AppearanceProfile,
    variant_id: &BodyVariantId,
) -> Vec<MorphTargetMapping> {
    profile
        .morph_mappings
        .iter()
        .filter(|mapping| mapping.variant_id == *variant_id && mapping.enabled)
        .cloned()
        .collect()
}

