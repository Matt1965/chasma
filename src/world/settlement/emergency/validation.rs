//! Emergency catalog / report validation (SA8).

use super::catalog::EmergencyCatalog;
use super::definition::EmergencyDefinition;
use crate::world::settlement::needs::NeedCatalog;
use crate::world::settlement::response::ResponseCatalog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmergencyValidationError {
    DuplicateEmergencyId(String),
    UnknownNeedId(String),
    UnknownResponseId(String),
    InvalidThresholds {
        emergency_id: String,
        detail: String,
    },
    InvalidModifierRange {
        emergency_id: String,
        detail: String,
    },
    NeverRecovers(String),
    BrokenEvaluator(String),
    ConflictingManualOverride(String),
}

impl std::fmt::Display for EmergencyValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateEmergencyId(id) => write!(f, "duplicate EmergencyId `{id}`"),
            Self::UnknownNeedId(id) => write!(f, "unknown NeedId `{id}`"),
            Self::UnknownResponseId(id) => write!(f, "unknown ResponseId `{id}`"),
            Self::InvalidThresholds {
                emergency_id,
                detail,
            } => write!(f, "invalid thresholds for `{emergency_id}`: {detail}"),
            Self::InvalidModifierRange {
                emergency_id,
                detail,
            } => write!(f, "invalid modifier for `{emergency_id}`: {detail}"),
            Self::NeverRecovers(id) => write!(f, "emergency `{id}` can never recover"),
            Self::BrokenEvaluator(id) => write!(f, "broken evaluator on `{id}`"),
            Self::ConflictingManualOverride(id) => {
                write!(f, "conflicting manual force+suppress on `{id}`")
            }
        }
    }
}

pub fn validate_emergency_catalog(
    catalog: &EmergencyCatalog,
    need_catalog: &NeedCatalog,
    response_catalog: &ResponseCatalog,
) -> Vec<EmergencyValidationError> {
    let mut errors = Vec::new();
    for def in catalog.definitions() {
        errors.extend(validate_emergency_definition(def, need_catalog, response_catalog));
    }
    errors
}

pub fn validate_emergency_definition(
    def: &EmergencyDefinition,
    need_catalog: &NeedCatalog,
    response_catalog: &ResponseCatalog,
) -> Vec<EmergencyValidationError> {
    let mut errors = Vec::new();
    let id = def.id.as_str();
    if !(0.0..=1.0).contains(&def.activation_threshold)
        || !(0.0..=1.0).contains(&def.deactivation_threshold)
    {
        errors.push(EmergencyValidationError::InvalidThresholds {
            emergency_id: id.into(),
            detail: "thresholds must be in 0..=1".into(),
        });
    }
    if def.deactivation_threshold >= def.activation_threshold {
        errors.push(EmergencyValidationError::InvalidThresholds {
            emergency_id: id.into(),
            detail: "deactivation must be < activation".into(),
        });
    }
    if def.deactivation_threshold <= 0.0 && def.activation_threshold >= 1.0 {
        errors.push(EmergencyValidationError::NeverRecovers(id.into()));
    }
    for m in &def.need_pressure_modifiers {
        if need_catalog.get(&m.need_id).is_none() {
            errors.push(EmergencyValidationError::UnknownNeedId(
                m.need_id.as_str().into(),
            ));
        }
        if !(-100.0..=100.0).contains(&m.pressure_delta_at_full) {
            errors.push(EmergencyValidationError::InvalidModifierRange {
                emergency_id: id.into(),
                detail: format!("pressure_delta {}", m.pressure_delta_at_full),
            });
        }
    }
    for m in &def.response_score_modifiers {
        if let Some(rid) = &m.response_id {
            if response_catalog.get(rid).is_none() {
                errors.push(EmergencyValidationError::UnknownResponseId(
                    rid.as_str().into(),
                ));
            }
        }
    }
    for rid in def
        .unlock_response_ids
        .iter()
        .chain(def.block_response_ids.iter())
    {
        if response_catalog.get(rid).is_none() {
            // Unlock may reference future responses — warn as unknown.
            errors.push(EmergencyValidationError::UnknownResponseId(
                rid.as_str().into(),
            ));
        }
    }
    let _ = EmergencyValidationError::BrokenEvaluator; // evaluator is enum — always valid
    errors
}
