//! Portrait-specific appearance and render-key resolution.

use crate::world::{
    AppearanceProfileCatalog, UnitAppearance, UnitDefinition, UnitRecord,
    definition_has_appearance_support, effective_render_key_for_appearance,
    effective_unit_render_key_str, resolve_canonical_default_appearance,
};

/// Appearance used to build the portrait actor (record or canonical default).
pub fn portrait_appearance_for_unit(
    unit: &UnitRecord,
    definition: &UnitDefinition,
    profiles: &AppearanceProfileCatalog,
) -> Option<UnitAppearance> {
    if let Some(appearance) = unit.appearance.clone() {
        return Some(appearance);
    }
    if definition_has_appearance_support(definition) {
        return resolve_canonical_default_appearance(definition, profiles).ok();
    }
    None
}

/// glTF scene stem for portrait capture (matches gameplay presentation authority).
pub fn portrait_render_key_str(
    unit: &UnitRecord,
    definition: &UnitDefinition,
    profiles: &AppearanceProfileCatalog,
) -> Option<String> {
    if definition_has_appearance_support(definition) {
        let appearance = portrait_appearance_for_unit(unit, definition, profiles)?;
        let key = effective_render_key_for_appearance(&appearance, profiles).ok()?;
        return key.0.filter(|key| !key.trim().is_empty());
    }
    effective_unit_render_key_str(unit, definition, profiles).ok()
}
