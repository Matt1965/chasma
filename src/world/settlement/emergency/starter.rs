//! Starter EmergencyDefinitions (SA8). Vertical slices only.

use super::definition::{
    EmergencyDefinition, EmergencyEvaluatorKind, EmergencyInterruptionPolicy, NeedPressureModifier,
    ResponseScoreModifier, TaskPriorityModifier,
};
use crate::world::settlement::needs::NeedId;
use crate::world::settlement::response::ResponseId;
use crate::world::task::TaskPriority;

pub fn starter_emergency_definitions() -> Vec<EmergencyDefinition> {
    vec![
        EmergencyDefinition::new(
            "starvation",
            "Starvation",
            "Food reserves critically low relative to settlement targets.",
            EmergencyEvaluatorKind::FoodReserveRatio,
            0.55,
            0.35,
        )
        .with_min_active(90)
        .with_need_pressure(NeedPressureModifier {
            need_id: NeedId::new("food"),
            category: None,
            pressure_delta_at_full: 45.0,
        })
        .with_response_score(ResponseScoreModifier {
            response_id: Some(ResponseId::new("increase_food_production")),
            response_tag: None,
            score_delta_at_full: 40.0,
        })
        .with_response_score(ResponseScoreModifier {
            response_id: Some(ResponseId::new("construct_food_building")),
            response_tag: None,
            score_delta_at_full: 35.0,
        })
        .with_block_tag("luxury")
        .with_block_tag("research")
        .with_task_bump("food")
        .with_interruption(EmergencyInterruptionPolicy {
            allow_interruption: true,
            min_stick_ticks: Some(20),
            min_priority_rank_gap: Some(1),
            max_interruptible_priority: Some(TaskPriority::Normal),
        }),
        EmergencyDefinition::new(
            "active_attack",
            "Active Attack",
            "Hostile presence threatens the settlement (signal-driven; no new combat AI).",
            EmergencyEvaluatorKind::HostilePresenceSignal,
            0.50,
            0.25,
        )
        .with_min_active(60)
        .with_need_pressure(NeedPressureModifier {
            need_id: NeedId::new("defense"),
            category: None,
            pressure_delta_at_full: 55.0,
        })
        .with_response_score(ResponseScoreModifier {
            response_id: Some(ResponseId::new("defend_settlement")),
            response_tag: None,
            score_delta_at_full: 50.0,
        })
        .with_unlock(ResponseId::new("defend_settlement_emergency"))
        .with_block_tag("luxury")
        .with_task_bump("defense")
        .with_interruption(EmergencyInterruptionPolicy {
            allow_interruption: true,
            min_stick_ticks: Some(15),
            min_priority_rank_gap: Some(1),
            max_interruptible_priority: Some(TaskPriority::Normal),
        }),
        EmergencyDefinition::new(
            "critical_fire",
            "Critical Fire",
            "Fire severity signal elevated (fixture/seam; no fire simulation).",
            EmergencyEvaluatorKind::FireSignal,
            0.60,
            0.30,
        )
        .with_min_active(45)
        .with_need_pressure(NeedPressureModifier {
            need_id: NeedId::new("housing"),
            category: None,
            pressure_delta_at_full: 35.0,
        })
        .with_need_pressure(NeedPressureModifier {
            need_id: NeedId::new("defense"),
            category: None,
            pressure_delta_at_full: 20.0,
        })
        .with_block_tag("luxury")
        .with_interruption(EmergencyInterruptionPolicy {
            allow_interruption: true,
            min_stick_ticks: Some(20),
            min_priority_rank_gap: Some(1),
            max_interruptible_priority: Some(TaskPriority::Low),
        }),
        EmergencyDefinition::new(
            "evacuation",
            "Evacuation",
            "Evacuate posture when evacuate_signal or compounded threat is high.",
            EmergencyEvaluatorKind::EvacuationSignal,
            0.70,
            0.40,
        )
        .with_min_active(120)
        .with_need_pressure(NeedPressureModifier {
            need_id: NeedId::new("defense"),
            category: None,
            pressure_delta_at_full: 40.0,
        })
        .with_need_pressure(NeedPressureModifier {
            need_id: NeedId::new("housing"),
            category: None,
            pressure_delta_at_full: 30.0,
        })
        .with_block_tag("luxury")
        .with_block_tag("research")
        .with_block_tag("expansion")
        .with_interruption(EmergencyInterruptionPolicy {
            allow_interruption: true,
            min_stick_ticks: Some(10),
            min_priority_rank_gap: Some(1),
            max_interruptible_priority: Some(TaskPriority::Normal),
        }),
    ]
}

trait EmergencyDefinitionBuilder {
    fn with_min_active(self, ticks: u64) -> Self;
    fn with_need_pressure(self, m: NeedPressureModifier) -> Self;
    fn with_response_score(self, m: ResponseScoreModifier) -> Self;
    fn with_unlock(self, id: ResponseId) -> Self;
    fn with_block_tag(self, tag: &str) -> Self;
    fn with_task_bump(self, tag: &str) -> Self;
    fn with_interruption(self, policy: EmergencyInterruptionPolicy) -> Self;
}

impl EmergencyDefinitionBuilder for EmergencyDefinition {
    fn with_min_active(mut self, ticks: u64) -> Self {
        self.min_active_duration_ticks = ticks;
        self
    }

    fn with_need_pressure(mut self, m: NeedPressureModifier) -> Self {
        self.need_pressure_modifiers.push(m);
        self
    }

    fn with_response_score(mut self, m: ResponseScoreModifier) -> Self {
        self.response_score_modifiers.push(m);
        self
    }

    fn with_unlock(mut self, id: ResponseId) -> Self {
        self.unlock_response_ids.push(id);
        self
    }

    fn with_block_tag(mut self, tag: &str) -> Self {
        self.block_response_tags.push(tag.into());
        self
    }

    fn with_task_bump(mut self, tag: &str) -> Self {
        self.task_priority_modifiers.push(TaskPriorityModifier {
            response_tag: Some(tag.into()),
            bump_one_tier: true,
        });
        self
    }

    fn with_interruption(mut self, policy: EmergencyInterruptionPolicy) -> Self {
        self.interruption = policy;
        self
    }
}
