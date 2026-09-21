//! Authoritative appearance commit for the Unit Editor (CG3).

use crate::world::{
    AppearanceProfileCatalog, UnitCatalog, UnitDefinition, UnitId, WorldData,
    validate_unit_appearance,
};

use super::draft::UnitAppearanceDraft;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitEditorCommitError {
    UnitMissing(UnitId),
    DefinitionMissing,
    ValidationFailed(String),
    WorldUpdateFailed,
}

impl std::fmt::Display for UnitEditorCommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnitMissing(id) => write!(f, "unit {} no longer exists", id.raw()),
            Self::DefinitionMissing => write!(f, "unit definition is missing"),
            Self::ValidationFailed(message) => write!(f, "{message}"),
            Self::WorldUpdateFailed => write!(f, "failed to update unit appearance"),
        }
    }
}

/// Validate and apply a draft appearance to a live unit through [`WorldData`].
pub fn commit_live_unit_appearance(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    profiles: &AppearanceProfileCatalog,
    unit_id: UnitId,
    draft: &UnitAppearanceDraft,
) -> Result<(), UnitEditorCommitError> {
    let record = world.get_unit(unit_id).ok_or(UnitEditorCommitError::UnitMissing(unit_id))?;
    let definition = catalog
        .get(&record.definition_id)
        .ok_or(UnitEditorCommitError::DefinitionMissing)?;
    validate_appearance_for_definition(draft, definition, profiles)?;
    world
        .mutate_unit(unit_id, |unit| {
            unit.appearance = Some(draft.appearance.clone());
        })
        .ok_or(UnitEditorCommitError::WorldUpdateFailed)?;
    Ok(())
}

pub fn validate_appearance_for_definition(
    draft: &UnitAppearanceDraft,
    definition: &UnitDefinition,
    profiles: &AppearanceProfileCatalog,
) -> Result<(), UnitEditorCommitError> {
    if draft.definition_id != definition.id {
        return Err(UnitEditorCommitError::ValidationFailed(
            "draft definition does not match unit definition".into(),
        ));
    }
    validate_unit_appearance(&draft.appearance, definition, profiles).map_err(|error| {
        UnitEditorCommitError::ValidationFailed(error.to_string())
    })
}
