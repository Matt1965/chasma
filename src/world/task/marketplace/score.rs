//! Assignment scoring for the Task marketplace (SA7).

use super::candidates::MarketplaceListing;
use crate::world::task::TaskPriority;

/// Minimum priority-rank gap required to preempt (lower rank = higher priority).
pub const MIN_PREEMPT_PRIORITY_RANKS: u8 = 1;
/// Worker must stick to a task this many ticks before preemption is allowed.
pub const MIN_STICK_TICKS: u64 = 45;
/// After a preemption, suppress further preemption for this many ticks (anti-thrash).
pub const PREEMPT_COOLDOWN_TICKS: u64 = 30;

/// Distance contribution scale (score subtracts distance * this).
const DISTANCE_WEIGHT: f32 = 0.05;
/// Base priority scores (higher = more attractive).
fn priority_score(priority: TaskPriority) -> f32 {
    match priority {
        TaskPriority::PlayerAssigned => 10_000.0,
        TaskPriority::High => 3_000.0,
        TaskPriority::Normal => 1_000.0,
        TaskPriority::Low => 300.0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssignmentScore {
    pub total: f32,
    pub priority_component: f32,
    pub distance_component: f32,
}

/// Score one worker↔listing pair. Higher is better.
pub fn score_marketplace_listing(
    listing: &MarketplaceListing,
    distance_meters: f32,
) -> AssignmentScore {
    let priority_component = priority_score(listing.priority);
    let distance = if distance_meters.is_finite() {
        distance_meters.max(0.0)
    } else {
        0.0
    };
    let distance_component = distance * DISTANCE_WEIGHT;
    AssignmentScore {
        total: priority_component - distance_component,
        priority_component,
        distance_component,
    }
}

/// Optional SA8 emergency relaxation of stick / rank gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PreemptPolicyOverride {
    pub min_stick_ticks: Option<u64>,
    pub min_priority_rank_gap: Option<u8>,
    /// Candidate may interrupt current only if current.rank() >= this ceiling's rank...
    /// Interpreted as: current priority is interruptible when `current.rank() >= max_interruptible.rank()`.
    pub max_interruptible: Option<TaskPriority>,
}

/// True when `candidate` may preempt a worker currently on `current` (priority + hysteresis).
pub fn may_preempt(
    candidate: TaskPriority,
    current: TaskPriority,
    ticks_on_current: u64,
    ticks_since_last_preempt: Option<u64>,
) -> bool {
    may_preempt_with_override(
        candidate,
        current,
        ticks_on_current,
        ticks_since_last_preempt,
        PreemptPolicyOverride::default(),
    )
}

pub fn may_preempt_with_override(
    candidate: TaskPriority,
    current: TaskPriority,
    ticks_on_current: u64,
    ticks_since_last_preempt: Option<u64>,
    policy: PreemptPolicyOverride,
) -> bool {
    if current == TaskPriority::PlayerAssigned {
        return false;
    }
    if let Some(max_irq) = policy.max_interruptible {
        // Can interrupt current only if current is at most as important as max_irq
        // (higher rank number = lower importance).
        if current.rank() < max_irq.rank() {
            return false;
        }
    }
    let min_stick = policy.min_stick_ticks.unwrap_or(MIN_STICK_TICKS);
    if ticks_on_current < min_stick {
        return false;
    }
    if let Some(since) = ticks_since_last_preempt {
        if since < PREEMPT_COOLDOWN_TICKS {
            return false;
        }
    }
    let min_gap = policy
        .min_priority_rank_gap
        .unwrap_or(MIN_PREEMPT_PRIORITY_RANKS);
    let gap = current.rank().saturating_sub(candidate.rank());
    gap >= min_gap
}
