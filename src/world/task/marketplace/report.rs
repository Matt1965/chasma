//! Transient worker assignment report (SA7). Never persisted.

use bevy::prelude::*;

use super::candidates::MarketplaceCandidate;
use crate::world::task::{TaskId, TaskPriority};
use crate::world::UnitId;

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
pub struct WorkerEvaluation {
    pub unit_id: UnitId,
    pub chosen_task_id: Option<TaskId>,
    pub chosen_score: f32,
    pub candidate_count: u32,
    pub top_candidates: Vec<String>,
    pub reservation_point: Option<String>,
    pub idle: bool,
    pub notes: String,
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
        Self {
            unit_id,
            chosen_task_id,
            chosen_score,
            candidate_count: candidates.len() as u32,
            top_candidates,
            reservation_point,
            idle,
            notes: notes.into(),
        }
    }
}
