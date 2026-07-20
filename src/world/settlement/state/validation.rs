//! SettlementState validation (SA1).

use super::types::{NeedCategory, SettlementKind, SettlementState};
use crate::world::settlement::{SettlementId, SettlementStore};
use crate::world::WorldData;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettlementStateValidationError {
    DuplicateSettlementId(SettlementId),
    OrphanState(SettlementId),
    MissingState(SettlementId),
    SettlementIdMismatch {
        key: SettlementId,
        state_id: SettlementId,
    },
    UnknownSettlementKind(String),
    InvalidNeedTarget {
        settlement_id: SettlementId,
        category: String,
        detail: String,
    },
    DuplicateNeedCategory {
        settlement_id: SettlementId,
        category: NeedCategory,
    },
    BrokenOwnership {
        settlement_id: SettlementId,
        detail: String,
    },
    InvalidPolicy {
        settlement_id: SettlementId,
        detail: String,
    },
    InvalidPlannerInterval {
        settlement_id: SettlementId,
        interval: u64,
    },
}

impl std::fmt::Display for SettlementStateValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateSettlementId(id) => {
                write!(f, "duplicate settlement state id {}", id.raw())
            }
            Self::OrphanState(id) => {
                write!(
                    f,
                    "settlement state {} has no SettlementRecord",
                    id.raw()
                )
            }
            Self::MissingState(id) => {
                write!(
                    f,
                    "SettlementRecord {} has no SettlementState",
                    id.raw()
                )
            }
            Self::SettlementIdMismatch { key, state_id } => write!(
                f,
                "settlement state key {} mismatches embedded id {}",
                key.raw(),
                state_id.raw()
            ),
            Self::UnknownSettlementKind(kind) => write!(f, "unknown settlement kind `{kind}`"),
            Self::InvalidNeedTarget {
                settlement_id,
                category,
                detail,
            } => write!(
                f,
                "settlement {} invalid need target `{category}`: {detail}",
                settlement_id.raw()
            ),
            Self::DuplicateNeedCategory {
                settlement_id,
                category,
            } => write!(
                f,
                "settlement {} duplicate need category {}",
                settlement_id.raw(),
                category.as_str()
            ),
            Self::BrokenOwnership {
                settlement_id,
                detail,
            } => write!(
                f,
                "settlement {} broken ownership: {detail}",
                settlement_id.raw()
            ),
            Self::InvalidPolicy {
                settlement_id,
                detail,
            } => write!(
                f,
                "settlement {} invalid policy: {detail}",
                settlement_id.raw()
            ),
            Self::InvalidPlannerInterval {
                settlement_id,
                interval,
            } => write!(
                f,
                "settlement {} planner interval must be > 0 (got {interval})",
                settlement_id.raw()
            ),
        }
    }
}

/// Validate a single SettlementState in isolation.
pub fn validate_settlement_state(state: &SettlementState) -> Vec<SettlementStateValidationError> {
    let mut errors = Vec::new();

    if SettlementKind::parse(state.kind.as_str()) != Some(state.kind) {
        errors.push(SettlementStateValidationError::UnknownSettlementKind(
            state.kind.as_str().to_string(),
        ));
    }

    if state.planner.evaluation_interval_ticks == 0 {
        errors.push(SettlementStateValidationError::InvalidPlannerInterval {
            settlement_id: state.settlement_id,
            interval: 0,
        });
    }

    if state.policies.response_preferences.keys().any(|k| k.is_empty()) {
        errors.push(SettlementStateValidationError::InvalidPolicy {
            settlement_id: state.settlement_id,
            detail: "empty response preference key".into(),
        });
    }

    let mut seen = std::collections::BTreeSet::new();
    for target in &state.need_targets {
        if !seen.insert(target.category) {
            errors.push(SettlementStateValidationError::DuplicateNeedCategory {
                settlement_id: state.settlement_id,
                category: target.category,
            });
        }
        if !target.weight.is_finite() || target.weight < 0.0 {
            errors.push(SettlementStateValidationError::InvalidNeedTarget {
                settlement_id: state.settlement_id,
                category: target.category.as_str().to_string(),
                detail: format!("weight must be finite and >= 0 (got {})", target.weight),
            });
        }
    }

    for modifier in &state.modifiers {
        if modifier.key.is_empty() {
            errors.push(SettlementStateValidationError::InvalidPolicy {
                settlement_id: state.settlement_id,
                detail: "modifier with empty key".into(),
            });
        }
        if !modifier.magnitude.is_finite() {
            errors.push(SettlementStateValidationError::InvalidPolicy {
                settlement_id: state.settlement_id,
                detail: format!("modifier `{}` magnitude is not finite", modifier.key),
            });
        }
    }

    errors
}

/// Validate SettlementStateStore against SettlementStore identity records.
pub fn validate_settlement_states(
    settlement_store: &SettlementStore,
    state_store: &super::store::SettlementStateStore,
) -> Vec<SettlementStateValidationError> {
    let mut errors = Vec::new();

    for (key, state) in state_store.iter() {
        if *key != state.settlement_id {
            errors.push(SettlementStateValidationError::SettlementIdMismatch {
                key: *key,
                state_id: state.settlement_id,
            });
        }
        if settlement_store.get_settlement(*key).is_none() {
            errors.push(SettlementStateValidationError::OrphanState(*key));
        }
        errors.extend(validate_settlement_state(state));
    }

    for id in settlement_store.sorted_settlement_ids() {
        if state_store.get(id).is_none() {
            errors.push(SettlementStateValidationError::MissingState(id));
        } else if let Some(record) = settlement_store.get_settlement(id) {
            // Anchor building must exist for ownership integrity when WorldData is available
            // via the world-level entry point below.
            let _ = record;
        }
    }

    errors
}

/// Full WorldData validation for settlement runtime.
pub fn validate_world_settlement_states(world: &WorldData) -> Vec<SettlementStateValidationError> {
    let mut errors = validate_settlement_states(
        world.settlement_store(),
        world.settlement_state_store(),
    );

    for id in world.settlement_store().sorted_settlement_ids() {
        let Some(record) = world.settlement_store().get_settlement(id) else {
            continue;
        };
        if world.get_building(record.anchor_building_id).is_none() {
            errors.push(SettlementStateValidationError::BrokenOwnership {
                settlement_id: id,
                detail: format!(
                    "anchor building {} missing",
                    record.anchor_building_id.raw()
                ),
            });
        }
        if world
            .settlement_store()
            .get_treasury(record.treasury_id)
            .is_none()
        {
            errors.push(SettlementStateValidationError::BrokenOwnership {
                settlement_id: id,
                detail: format!("treasury {} missing", record.treasury_id.raw()),
            });
        }
    }

    errors
}

/// Ensure every SettlementRecord has a SettlementState (defaults for missing).
pub fn ensure_settlement_states_for_world(world: &mut WorldData) {
    let ids: Vec<_> = world.settlement_store().sorted_settlement_ids();
    for id in ids {
        let player_controlled = world
            .settlement_store()
            .get_settlement(id)
            .map(|r| r.ownership.affiliation == crate::world::Affiliation::Player)
            .unwrap_or(false);
        world
            .settlement_state_store_mut()
            .ensure(id, SettlementKind::Town, player_controlled);
    }
}
