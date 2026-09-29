//! Bounded per-unit decision transition history (dev-only, non-authoritative).

use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;

use crate::world::{UnitId, UnitState, WorldData, unit_in_active_combat};

const MAX_HISTORY_PER_UNIT: usize = 8;

#[derive(Resource, Debug, Default)]
pub struct BrainDecisionHistory {
    entries: HashMap<UnitId, VecDeque<BrainHistoryEntry>>,
    previous: HashMap<UnitId, BrainUnitSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainHistoryEntry {
    pub tick: u64,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrainUnitSignal {
    task_id: Option<u32>,
    combat_active: bool,
    state_tag: &'static str,
}

/// Records meaningful transitions: task assignment, combat enter/exit, working start/stop.
pub fn record_brain_unit_history(world: &WorldData, tick: u64, history: &mut BrainDecisionHistory) {
    for unit_id in world.sorted_unit_ids() {
        let Some(unit) = world.get_unit(unit_id) else {
            continue;
        };
        let signal = BrainUnitSignal {
            task_id: world.task_store().unit_task_id(unit_id).map(|id| id.raw()),
            combat_active: unit_in_active_combat(&unit.combat_state),
            state_tag: state_tag(&unit.state),
        };
        let previous = history.previous.get(&unit_id);
        if previous == Some(&signal) {
            continue;
        }
        if let Some(prev) = previous {
            if let Some(line) = transition_line(prev, &signal, tick) {
                push_history(history, unit_id, tick, line);
            }
        }
        history.previous.insert(unit_id, signal);
    }
}

fn state_tag(state: &UnitState) -> &'static str {
    match state {
        UnitState::Idle => "idle",
        UnitState::Moving { .. } => "moving",
        UnitState::Working { .. } => "working",
        UnitState::Dead => "dead",
    }
}

fn transition_line(prev: &BrainUnitSignal, next: &BrainUnitSignal, tick: u64) -> Option<String> {
    if prev.combat_active != next.combat_active {
        return Some(if next.combat_active {
            format!("tick {}: combat override", tick)
        } else {
            format!("tick {}: combat ended", tick)
        });
    }
    if prev.task_id != next.task_id {
        return Some(format!(
            "tick {}: task {:?} -> {:?}",
            tick,
            prev.task_id,
            next.task_id
        ));
    }
    if prev.state_tag != next.state_tag
        && (prev.state_tag == "working" || next.state_tag == "working")
    {
        return Some(format!(
            "tick {}: activity {} -> {}",
            tick,
            prev.state_tag,
            next.state_tag
        ));
    }
    None
}

fn push_history(history: &mut BrainDecisionHistory, unit_id: UnitId, tick: u64, summary: String) {
    let deque = history.entries.entry(unit_id).or_default();
    if deque.len() >= MAX_HISTORY_PER_UNIT {
        deque.pop_front();
    }
    deque.push_back(BrainHistoryEntry { tick, summary });
}

pub fn tick_brain_decision_history(
    dev_state: Res<crate::dev::dev_mode::DevModeState>,
    simulation: Res<crate::simulation::SimulationControlState>,
    world: Res<WorldData>,
    mut history: ResMut<BrainDecisionHistory>,
) {
    if !dev_state.enabled {
        return;
    }
    record_brain_unit_history(&world, simulation.current_tick, &mut history);
}

impl BrainDecisionHistory {
    pub fn lines_for(&self, unit_id: UnitId) -> Vec<String> {
        self.entries
            .get(&unit_id)
            .map(|q| q.iter().map(|e| e.summary.clone()).collect())
            .unwrap_or_default()
    }

    pub fn len_for(&self, unit_id: UnitId) -> usize {
        self.entries.get(&unit_id).map(|q| q.len()).unwrap_or(0)
    }

    pub fn clear_unit(&mut self, unit_id: UnitId) {
        self.entries.remove(&unit_id);
        self.previous.remove(&unit_id);
    }
}
