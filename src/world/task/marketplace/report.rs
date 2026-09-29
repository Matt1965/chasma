//! Transient worker assignment report (SA7). Never persisted.

use bevy::prelude::*;

use super::candidates::{MarketplaceCandidate, MarketplaceListing};
use crate::world::UnitId;
use crate::world::task::{TaskId, TaskPriority, TaskType};

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct AssignmentDecision {
    pub unit_id: UnitId,
    pub task_id: Option<TaskId>,
    pub score: f32,
    pub priority: TaskPriority,
    pub preempted: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct WorkerCandidateDiagnostic {
    pub task_id: Option<TaskId>,
    pub task_type: TaskType,
    pub priority: TaskPriority,
    pub distance_meters: f32,
    pub priority_component: f32,
    pub distance_component: f32,
    pub total_score: f32,
    pub eligible: bool,
    pub block_reason: Option<String>,
    pub chosen: bool,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct WorkerEvaluation {
    pub unit_id: UnitId,
    pub chosen_task_id: Option<TaskId>,
    pub chosen_score: f32,
    pub candidate_count: u32,
    pub top_candidates: Vec<String>,
    /// Structured SA7 race rows (authoritative scoring fields from marketplace step).
    pub ranked_candidates: Vec<WorkerCandidateDiagnostic>,
    pub reservation_point: Option<String>,
    pub idle: bool,
    pub notes: String,
}

impl WorkerCandidateDiagnostic {
    pub fn from_marketplace_candidate(
        listing: &MarketplaceListing,
        distance_meters: f32,
        priority_component: f32,
        distance_component: f32,
        total_score: f32,
        eligible: bool,
        block_reason: Option<String>,
        chosen: bool,
    ) -> Self {
        Self {
            task_id: listing.task_id,
            task_type: listing.task_type,
            priority: listing.priority,
            distance_meters,
            priority_component,
            distance_component,
            total_score,
            eligible,
            block_reason,
            chosen,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct WorkerAssignmentReport {
    pub generated_tick: u64,
    pub idle_workers: u32,
    pub open_listings: u32,
    pub assignments: Vec<AssignmentDecision>,
    pub evaluations: Vec<WorkerEvaluation>,
    pub diagnostics: Vec<String>,
}

impl Default for WorkerAssignmentReport {
    fn default() -> Self {
        Self {
            generated_tick: 0,
            idle_workers: 0,
            open_listings: 0,
            assignments: Vec::new(),
            evaluations: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

impl WorkerEvaluation {
    pub fn from_candidates(
        unit_id: UnitId,
        idle: bool,
        candidates: &[MarketplaceCandidate],
        chosen_task_id: Option<TaskId>,
        chosen_score: f32,
        reservation_point: Option<String>,
        notes: impl Into<String>,
    ) -> Self {
        let mut ranked: Vec<_> = candidates.iter().filter(|c| c.eligible).collect();
        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.listing
                        .task_id
                        .map(|id| id.raw())
                        .cmp(&b.listing.task_id.map(|id| id.raw()))
                })
        });
        let top_candidates = ranked
            .iter()
            .take(5)
            .map(|c| {
                format!(
                    "{:?} pri={:?} dist={:.1} score={:.1}{}",
                    c.listing.task_type,
                    c.listing.priority,
                    c.distance_meters,
                    c.score,
                    c.block_reason
                        .as_ref()
                        .map(|r| format!(" ({r})"))
                        .unwrap_or_default()
                )
            })
            .collect();
        let mut ranked_candidates: Vec<WorkerCandidateDiagnostic> = candidates
            .iter()
            .map(|c| {
                WorkerCandidateDiagnostic::from_marketplace_candidate(
                    &c.listing,
                    c.distance_meters,
                    c.priority_component,
                    c.distance_component,
                    c.score,
                    c.eligible,
                    c.block_reason.clone(),
                    chosen_task_id.is_some() && c.listing.task_id == chosen_task_id,
                )
            })
            .collect();
        ranked_candidates.sort_by(|a, b| {
            b.total_score
                .partial_cmp(&a.total_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.task_id.map(|id| id.raw()).cmp(&b.task_id.map(|id| id.raw())))
        });
        Self {
            unit_id,
            chosen_task_id,
            chosen_score,
            candidate_count: candidates.len() as u32,
            top_candidates,
            ranked_candidates,
            reservation_point,
            idle,
            notes: notes.into(),
        }
    }
}
