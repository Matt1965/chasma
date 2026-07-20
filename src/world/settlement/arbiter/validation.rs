//! SettlementIntent validation (SA4).

use std::collections::BTreeSet;

use super::intent::{SettlementIntent, SettlementIntentPlan};
use crate::world::settlement::response::ResponseCatalog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentValidationError {
    DuplicateIntentId(String),
    DuplicateResponseNeed {
        response_id: String,
        need_id: String,
    },
    UnknownResponse(String),
    InvalidPriority {
        intent_id: String,
        detail: String,
    },
    ConflictingTypes {
        need_id: String,
        detail: String,
    },
    BrokenReference {
        intent_id: String,
        detail: String,
    },
}

impl std::fmt::Display for IntentValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateIntentId(id) => write!(f, "duplicate IntentId `{id}`"),
            Self::DuplicateResponseNeed {
                response_id,
                need_id,
            } => write!(
                f,
                "duplicate intent response=`{response_id}` need=`{need_id}`"
            ),
            Self::UnknownResponse(id) => write!(f, "unknown ResponseId `{id}`"),
            Self::InvalidPriority { intent_id, detail } => {
                write!(f, "intent `{intent_id}` invalid priority: {detail}")
            }
            Self::ConflictingTypes { need_id, detail } => {
                write!(f, "need `{need_id}` conflicting response types: {detail}")
            }
            Self::BrokenReference { intent_id, detail } => {
                write!(f, "intent `{intent_id}` broken reference: {detail}")
            }
        }
    }
}

pub fn validate_intent(
    intent: &SettlementIntent,
    catalog: Option<&ResponseCatalog>,
) -> Vec<IntentValidationError> {
    let mut errors = Vec::new();
    if intent.intent_id.as_str().is_empty() {
        errors.push(IntentValidationError::BrokenReference {
            intent_id: String::new(),
            detail: "empty IntentId".into(),
        });
    }
    if intent.source_need.as_str().is_empty() {
        errors.push(IntentValidationError::BrokenReference {
            intent_id: intent.intent_id.as_str().to_string(),
            detail: "empty source need".into(),
        });
    }
    if intent.chosen_response.as_str().is_empty() {
        errors.push(IntentValidationError::BrokenReference {
            intent_id: intent.intent_id.as_str().to_string(),
            detail: "empty chosen response".into(),
        });
    }
    if !intent.priority.is_finite() || intent.priority < 0.0 {
        errors.push(IntentValidationError::InvalidPriority {
            intent_id: intent.intent_id.as_str().to_string(),
            detail: format!("{}", intent.priority),
        });
    }
    if let Some(catalog) = catalog {
        if catalog.get(&intent.chosen_response).is_none() {
            errors.push(IntentValidationError::UnknownResponse(
                intent.chosen_response.as_str().to_string(),
            ));
        }
    }
    errors
}

pub fn validate_settlement_intent_plan(
    plan: &SettlementIntentPlan,
    catalog: Option<&ResponseCatalog>,
) -> Vec<IntentValidationError> {
    let mut errors = Vec::new();
    let mut seen_ids = BTreeSet::new();
    let mut seen_pairs = BTreeSet::new();
    let mut types_by_need: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();

    for intent in &plan.intents {
        if !seen_ids.insert(intent.intent_id.as_str().to_string()) {
            errors.push(IntentValidationError::DuplicateIntentId(
                intent.intent_id.as_str().to_string(),
            ));
        }
        let pair = (
            intent.chosen_response.as_str().to_string(),
            intent.source_need.as_str().to_string(),
        );
        if !seen_pairs.insert(pair.clone()) {
            errors.push(IntentValidationError::DuplicateResponseNeed {
                response_id: pair.0,
                need_id: pair.1,
            });
        }
        types_by_need
            .entry(intent.source_need.as_str().to_string())
            .or_default()
            .push(intent.response_type.as_str().to_string());
        errors.extend(validate_intent(intent, catalog));
    }

    for (need_id, types) in types_by_need {
        let has_inc = types.iter().any(|t| t == "increase_production");
        let has_dec = types.iter().any(|t| t == "decrease_production");
        if has_inc && has_dec {
            errors.push(IntentValidationError::ConflictingTypes {
                need_id,
                detail: "increase_production and decrease_production".into(),
            });
        }
    }

    errors
}
