//! Need catalog and snapshot validation (SA2).

use super::catalog::NeedCatalog;
use super::definition::NeedEvaluationMethod;
use super::id::NeedId;
use super::snapshot::{NeedSnapshot, SettlementNeedEvaluation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NeedCatalogError {
    EmptyNeedId,
    DuplicateNeedId(NeedId),
    UnknownEvaluator(NeedEvaluationMethod),
}

impl std::fmt::Display for NeedCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyNeedId => write!(f, "need definition has empty id"),
            Self::DuplicateNeedId(id) => write!(f, "duplicate NeedId `{}`", id.as_str()),
            Self::UnknownEvaluator(method) => {
                write!(f, "unknown need evaluator `{method:?}`")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NeedEvaluationValidationError {
    PressureOutOfRange {
        need_id: String,
        pressure: u8,
    },
    BrokenNormalization {
        need_id: String,
        detail: String,
    },
    InvalidTarget {
        need_id: String,
        detail: String,
    },
    DuplicateSnapshot {
        need_id: String,
    },
}

impl std::fmt::Display for NeedEvaluationValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PressureOutOfRange { need_id, pressure } => {
                write!(f, "need `{need_id}` pressure {pressure} outside 0..=100")
            }
            Self::BrokenNormalization { need_id, detail } => {
                write!(f, "need `{need_id}` broken normalization: {detail}")
            }
            Self::InvalidTarget { need_id, detail } => {
                write!(f, "need `{need_id}` invalid target: {detail}")
            }
            Self::DuplicateSnapshot { need_id } => {
                write!(f, "duplicate snapshot for need `{need_id}`")
            }
        }
    }
}

/// Validate a single snapshot's pressure and numeric integrity.
pub fn validate_need_snapshot(snapshot: &NeedSnapshot) -> Vec<NeedEvaluationValidationError> {
    let mut errors = Vec::new();
    if snapshot.pressure > 100 {
        errors.push(NeedEvaluationValidationError::PressureOutOfRange {
            need_id: snapshot.need_id.as_str().to_string(),
            pressure: snapshot.pressure,
        });
    }
    if !snapshot.current_value.is_finite() || !snapshot.desired_value.is_finite() {
        errors.push(NeedEvaluationValidationError::BrokenNormalization {
            need_id: snapshot.need_id.as_str().to_string(),
            detail: "non-finite current/desired".into(),
        });
    }
    if snapshot.desired_value < 0.0 {
        errors.push(NeedEvaluationValidationError::InvalidTarget {
            need_id: snapshot.need_id.as_str().to_string(),
            detail: format!("desired {} < 0", snapshot.desired_value),
        });
    }
    let expected_deficit = (snapshot.desired_value - snapshot.current_value).max(0.0);
    if (snapshot.deficit - expected_deficit).abs() > 0.01 {
        errors.push(NeedEvaluationValidationError::BrokenNormalization {
            need_id: snapshot.need_id.as_str().to_string(),
            detail: format!(
                "deficit {} != expected {}",
                snapshot.deficit, expected_deficit
            ),
        });
    }
    errors
}

pub fn validate_settlement_need_evaluation(
    evaluation: &SettlementNeedEvaluation,
) -> Vec<NeedEvaluationValidationError> {
    let mut errors = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for snapshot in &evaluation.snapshots {
        if !seen.insert(snapshot.need_id.as_str().to_string()) {
            errors.push(NeedEvaluationValidationError::DuplicateSnapshot {
                need_id: snapshot.need_id.as_str().to_string(),
            });
        }
        errors.extend(validate_need_snapshot(snapshot));
    }
    errors
}

pub fn validate_need_catalog(catalog: &NeedCatalog) -> Vec<NeedCatalogError> {
    // Catalog construction already validates; re-check for Dev Mode.
    let mut errors = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for def in catalog.definitions() {
        if def.id.as_str().is_empty() {
            errors.push(NeedCatalogError::EmptyNeedId);
        }
        if !seen.insert(def.id.as_str().to_string()) {
            errors.push(NeedCatalogError::DuplicateNeedId(def.id.clone()));
        }
    }
    errors
}
