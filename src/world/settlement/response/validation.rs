//! Response catalog and candidate validation (SA3).

use std::collections::{BTreeSet, HashMap, HashSet};

use super::candidate::{CandidateResponse, SettlementResponseCandidates};
use super::definition::{CapabilityRequirement, ResponseDefinition};
use super::id::ResponseId;
use crate::world::settlement::needs::{NeedCatalog, NeedId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseCatalogError {
    EmptyResponseId,
    DuplicateResponseId(ResponseId),
    UnknownNeedId(NeedId),
    EmptySupportedNeeds(ResponseId),
    InvalidExpectedEffect {
        response_id: ResponseId,
        detail: String,
    },
    InvalidCapability(ResponseId, String),
    CircularPrerequisites(Vec<ResponseId>),
    UnknownPrerequisite {
        response_id: ResponseId,
        prerequisite: ResponseId,
    },
}

impl std::fmt::Display for ResponseCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyResponseId => write!(f, "response definition has empty id"),
            Self::DuplicateResponseId(id) => {
                write!(f, "duplicate ResponseId `{}`", id.as_str())
            }
            Self::UnknownNeedId(id) => write!(f, "unknown NeedId `{}`", id.as_str()),
            Self::EmptySupportedNeeds(id) => {
                write!(f, "response `{}` supports no needs", id.as_str())
            }
            Self::InvalidExpectedEffect { response_id, detail } => {
                write!(
                    f,
                    "response `{}` invalid expected effect: {detail}",
                    response_id.as_str()
                )
            }
            Self::InvalidCapability(id, detail) => {
                write!(f, "response `{}` invalid capability: {detail}", id.as_str())
            }
            Self::CircularPrerequisites(cycle) => {
                let path: Vec<&str> = cycle.iter().map(|id| id.as_str()).collect();
                write!(f, "circular response prerequisites: {}", path.join(" -> "))
            }
            Self::UnknownPrerequisite {
                response_id,
                prerequisite,
            } => write!(
                f,
                "response `{}` references unknown prerequisite `{}`",
                response_id.as_str(),
                prerequisite.as_str()
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseCandidateValidationError {
    NegativeScore {
        response_id: String,
        score: String,
    },
    NonFiniteScore {
        response_id: String,
    },
    AvailableWithBlocking {
        response_id: String,
    },
    DuplicateCandidate {
        response_id: String,
        need_id: String,
    },
}

impl std::fmt::Display for ResponseCandidateValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NegativeScore { response_id, score } => {
                write!(f, "candidate `{response_id}` has negative score {score}")
            }
            Self::NonFiniteScore { response_id } => {
                write!(f, "candidate `{response_id}` has non-finite score")
            }
            Self::AvailableWithBlocking { response_id } => {
                write!(
                    f,
                    "candidate `{response_id}` marked available but has blocking reason"
                )
            }
            Self::DuplicateCandidate {
                response_id,
                need_id,
            } => write!(
                f,
                "duplicate candidate response=`{response_id}` need=`{need_id}`"
            ),
        }
    }
}

/// Validate definitions before building a catalog. Need ids checked against NeedCatalog when provided.
pub fn validate_response_catalog_definitions(
    definitions: &[ResponseDefinition],
) -> Result<(), ResponseCatalogError> {
    validate_response_catalog_definitions_with_needs(definitions, None)
}

pub fn validate_response_catalog_definitions_with_needs(
    definitions: &[ResponseDefinition],
    need_catalog: Option<&NeedCatalog>,
) -> Result<(), ResponseCatalogError> {
    let mut seen = HashSet::new();
    let mut by_id: HashMap<ResponseId, &ResponseDefinition> = HashMap::new();

    for def in definitions {
        if def.id.as_str().is_empty() {
            return Err(ResponseCatalogError::EmptyResponseId);
        }
        if !seen.insert(def.id.clone()) {
            return Err(ResponseCatalogError::DuplicateResponseId(def.id.clone()));
        }
        if def.supported_need_ids.is_empty() {
            return Err(ResponseCatalogError::EmptySupportedNeeds(def.id.clone()));
        }
        for need_id in &def.supported_need_ids {
            if need_id.as_str().is_empty() {
                return Err(ResponseCatalogError::UnknownNeedId(need_id.clone()));
            }
            if let Some(needs) = need_catalog {
                if needs.get(need_id).is_none() {
                    return Err(ResponseCatalogError::UnknownNeedId(need_id.clone()));
                }
            }
        }
        let effect = &def.expected_effect;
        if !effect.pressure_relief.is_finite()
            || effect.pressure_relief < 0.0
            || effect.pressure_relief > 1.0
        {
            return Err(ResponseCatalogError::InvalidExpectedEffect {
                response_id: def.id.clone(),
                detail: format!("pressure_relief {}", effect.pressure_relief),
            });
        }
        if !effect.estimated_cost.is_finite() || effect.estimated_cost < 0.0 {
            return Err(ResponseCatalogError::InvalidExpectedEffect {
                response_id: def.id.clone(),
                detail: format!("estimated_cost {}", effect.estimated_cost),
            });
        }
        for req in &def.capability_requirements {
            validate_capability_shape(&def.id, req)?;
        }
        by_id.insert(def.id.clone(), def);
    }

    for def in definitions {
        for prereq in &def.prerequisite_response_ids {
            if !by_id.contains_key(prereq) {
                return Err(ResponseCatalogError::UnknownPrerequisite {
                    response_id: def.id.clone(),
                    prerequisite: prereq.clone(),
                });
            }
        }
    }

    if let Some(cycle) = find_prerequisite_cycle(definitions) {
        return Err(ResponseCatalogError::CircularPrerequisites(cycle));
    }

    Ok(())
}

fn validate_capability_shape(
    response_id: &ResponseId,
    req: &CapabilityRequirement,
) -> Result<(), ResponseCatalogError> {
    match req {
        CapabilityRequirement::SupportingOperation(op) if op.is_empty() => Err(
            ResponseCatalogError::InvalidCapability(response_id.clone(), "empty operation id".into()),
        ),
        CapabilityRequirement::BuildingDefinition(id) if id.is_empty() => {
            Err(ResponseCatalogError::InvalidCapability(
                response_id.clone(),
                "empty building definition id".into(),
            ))
        }
        _ => Ok(()),
    }
}

fn find_prerequisite_cycle(definitions: &[ResponseDefinition]) -> Option<Vec<ResponseId>> {
    let mut graph: HashMap<&ResponseId, &[ResponseId]> = HashMap::new();
    for def in definitions {
        graph.insert(&def.id, &def.prerequisite_response_ids);
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    let mut stack = Vec::new();

    for def in definitions {
        if let Some(cycle) = dfs_cycle(
            &def.id,
            &graph,
            &mut visiting,
            &mut visited,
            &mut stack,
        ) {
            return Some(cycle);
        }
    }
    None
}

fn dfs_cycle<'a>(
    node: &'a ResponseId,
    graph: &HashMap<&'a ResponseId, &'a [ResponseId]>,
    visiting: &mut HashSet<&'a ResponseId>,
    visited: &mut HashSet<&'a ResponseId>,
    stack: &mut Vec<&'a ResponseId>,
) -> Option<Vec<ResponseId>> {
    if visited.contains(node) {
        return None;
    }
    if !visiting.insert(node) {
        let start = stack.iter().position(|id| *id == node).unwrap_or(0);
        let mut cycle: Vec<ResponseId> = stack[start..].iter().map(|id| (*id).clone()).collect();
        cycle.push(node.clone());
        return Some(cycle);
    }
    stack.push(node);
    if let Some(edges) = graph.get(node) {
        for next in *edges {
            if let Some(cycle) = dfs_cycle(next, graph, visiting, visited, stack) {
                return Some(cycle);
            }
        }
    }
    stack.pop();
    visiting.remove(node);
    visited.insert(node);
    None
}

pub fn validate_candidate(candidate: &CandidateResponse) -> Vec<ResponseCandidateValidationError> {
    let mut errors = Vec::new();
    if !candidate.priority_score.is_finite() {
        errors.push(ResponseCandidateValidationError::NonFiniteScore {
            response_id: candidate.response_id.as_str().to_string(),
        });
    } else if candidate.priority_score < 0.0 {
        errors.push(ResponseCandidateValidationError::NegativeScore {
            response_id: candidate.response_id.as_str().to_string(),
            score: candidate.priority_score.to_string(),
        });
    }
    if candidate.is_available() && candidate.blocking_reason.is_some() {
        errors.push(ResponseCandidateValidationError::AvailableWithBlocking {
            response_id: candidate.response_id.as_str().to_string(),
        });
    }
    errors
}

pub fn validate_settlement_response_candidates(
    result: &SettlementResponseCandidates,
) -> Vec<ResponseCandidateValidationError> {
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    for candidate in &result.candidates {
        let key = (
            candidate.response_id.as_str().to_string(),
            candidate.need_id.as_str().to_string(),
        );
        if !seen.insert(key.clone()) {
            errors.push(ResponseCandidateValidationError::DuplicateCandidate {
                response_id: key.0,
                need_id: key.1,
            });
        }
        errors.extend(validate_candidate(candidate));
    }
    errors
}

/// Dev-mode revalidation of a built catalog against NeedCatalog.
pub fn validate_response_catalog_against_needs(
    definitions: &[ResponseDefinition],
    need_catalog: &NeedCatalog,
) -> Vec<ResponseCatalogError> {
    match validate_response_catalog_definitions_with_needs(definitions, Some(need_catalog)) {
        Ok(()) => Vec::new(),
        Err(err) => vec![err],
    }
}
