//! Persistent SettlementState types (SA1). Storage only — no evaluation or planning.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::settlement::SettlementId;

/// Authored settlement archetype. Defaults only — never hardcoded behavior branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SettlementKind {
    #[default]
    Town,
    Village,
    Hive,
    Pack,
    Herd,
    Camp,
    Outpost,
}

impl SettlementKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Town => "town",
            Self::Village => "village",
            Self::Hive => "hive",
            Self::Pack => "pack",
            Self::Herd => "herd",
            Self::Camp => "camp",
            Self::Outpost => "outpost",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "town" => Some(Self::Town),
            "village" => Some(Self::Village),
            "hive" => Some(Self::Hive),
            "pack" => Some(Self::Pack),
            "herd" => Some(Self::Herd),
            "camp" => Some(Self::Camp),
            "outpost" => Some(Self::Outpost),
            _ => None,
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Town,
            Self::Village,
            Self::Hive,
            Self::Pack,
            Self::Herd,
            Self::Camp,
            Self::Outpost,
        ]
    }
}

/// Long-term configuration intent. Policies do not execute behavior.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct SettlementPolicies {
    /// 0 = passive … 255 = maximum aggression. Authored intent only.
    pub aggression: u8,
    pub expansion_enabled: bool,
    pub automation_enabled: bool,
    pub planner_enabled: bool,
    /// Player may override policies; runtime structure is identical for AI settlements.
    pub player_controlled: bool,
    /// When false, automatic emergency activation from signals is suppressed (manual still allowed).
    #[serde(default = "default_true")]
    pub auto_emergency_response: bool,
    /// When false, emergency production/response score reweighting is skipped.
    #[serde(default = "default_true")]
    pub auto_production_reprioritize: bool,
    /// When false, emergency-driven task interruption is disabled.
    #[serde(default = "default_true")]
    pub auto_task_interruption: bool,
    /// When false, autonomous ConstructionPlan creation from intent is suppressed (SA9).
    #[serde(default = "default_true")]
    pub auto_construction: bool,
    /// Player settlements may require explicit plan approval before site commit (SA9).
    #[serde(default)]
    pub require_construction_approval: bool,
    /// Player settlements may require explicit placement approval after site search (SA9).
    #[serde(default)]
    pub require_construction_placement_approval: bool,
    /// Maximum active construction plans for this settlement.
    #[serde(default = "default_max_concurrent_construction_plans")]
    pub max_concurrent_construction_plans: u32,
    /// Maximum new plans created in one planning pass.
    #[serde(default = "default_max_new_construction_plans_per_cycle")]
    pub max_new_construction_plans_per_cycle: u32,
    /// Placement search radius from settlement anchor (meters).
    #[serde(default = "default_construction_search_radius_meters")]
    pub construction_search_radius_meters: f32,
    /// Max placement candidates evaluated per planning pass.
    #[serde(default = "default_max_placement_candidates_per_pass")]
    pub max_placement_candidates_per_pass: u32,
    /// Retry cadence for blocked plans (ticks).
    #[serde(default = "default_blocked_plan_retry_ticks")]
    pub blocked_plan_retry_ticks: u64,
    /// Future response preference keys (data seam).
    #[serde(default)]
    pub response_preferences: BTreeMap<String, String>,
}

fn default_true() -> bool {
    true
}

fn default_max_concurrent_construction_plans() -> u32 {
    3
}

fn default_max_new_construction_plans_per_cycle() -> u32 {
    1
}

fn default_construction_search_radius_meters() -> f32 {
    48.0
}

fn default_max_placement_candidates_per_pass() -> u32 {
    24
}

fn default_blocked_plan_retry_ticks() -> u64 {
    60
}

impl Default for SettlementPolicies {
    fn default() -> Self {
        Self {
            aggression: 64,
            expansion_enabled: true,
            automation_enabled: true,
            planner_enabled: true,
            player_controlled: false,
            auto_emergency_response: true,
            auto_production_reprioritize: true,
            auto_task_interruption: true,
            auto_construction: true,
            require_construction_approval: false,
            require_construction_placement_approval: false,
            max_concurrent_construction_plans: default_max_concurrent_construction_plans(),
            max_new_construction_plans_per_cycle: default_max_new_construction_plans_per_cycle(),
            construction_search_radius_meters: default_construction_search_radius_meters(),
            max_placement_candidates_per_pass: default_max_placement_candidates_per_pass(),
            blocked_plan_retry_ticks: default_blocked_plan_retry_ticks(),
            response_preferences: BTreeMap::new(),
        }
    }
}

impl SettlementPolicies {
    pub fn for_player() -> Self {
        Self {
            player_controlled: true,
            ..Self::default()
        }
    }
}

/// Need categories. Targets are authored; pressure/shortage computation is SA2+.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedCategory {
    Food,
    Water,
    Housing,
    Defense,
    Construction,
    Research,
    Luxury,
    Medicine,
    Population,
    Economy,
    Growth,
}

impl NeedCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Food => "food",
            Self::Water => "water",
            Self::Housing => "housing",
            Self::Defense => "defense",
            Self::Construction => "construction",
            Self::Research => "research",
            Self::Luxury => "luxury",
            Self::Medicine => "medicine",
            Self::Population => "population",
            Self::Economy => "economy",
            Self::Growth => "growth",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "food" => Some(Self::Food),
            "water" => Some(Self::Water),
            "housing" => Some(Self::Housing),
            "defense" => Some(Self::Defense),
            "construction" => Some(Self::Construction),
            "research" => Some(Self::Research),
            "luxury" => Some(Self::Luxury),
            "medicine" => Some(Self::Medicine),
            "population" => Some(Self::Population),
            "economy" => Some(Self::Economy),
            "growth" => Some(Self::Growth),
            _ => None,
        }
    }
}

/// Authored need target. No shortage or pressure fields — those are derived (SA2).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct NeedTarget {
    pub category: NeedCategory,
    /// Desired level for this need (units are category-defined by future SA phases).
    pub target_value: u32,
    /// Relative importance weight for future arbitration (storage only in SA1).
    pub weight: f32,
}

impl NeedTarget {
    pub fn new(category: NeedCategory, target_value: u32, weight: f32) -> Self {
        Self {
            category,
            target_value,
            weight,
        }
    }
}

/// Source of a persistent modifier nudge.
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementModifierSource {
    Faction,
    Player,
    Difficulty,
    Scenario,
    Traits,
    Weather,
    Events,
    /// Forward-compatible extension key.
    Extension(String),
}

/// Persistent modifier storage. Future systems adjust these; SA1 only stores them.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct SettlementModifier {
    pub source: SettlementModifierSource,
    pub key: String,
    pub magnitude: f32,
    /// Optional expiry tick; `None` means permanent until cleared.
    pub expires_tick: Option<u64>,
}

/// One active emergency instance (authoritative continuity — SA8 / ADR-123).
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct ActiveEmergencyInstance {
    pub emergency_id: String,
    pub activated_tick: u64,
    /// Normalized severity `0.0..=1.0`.
    pub severity: f32,
    /// Last evaluated detection signal `0.0..=1.0` (diagnostic continuity).
    #[serde(default)]
    pub last_signal: f32,
    /// Player/AI forced active regardless of signal.
    #[serde(default)]
    pub manual_force: bool,
    /// Player/AI suppression — prevents automatic activation while set.
    #[serde(default)]
    pub manual_suppress: bool,
    #[serde(default)]
    pub acknowledged: bool,
    /// Opaque source reference (sensor key / fixture id).
    #[serde(default)]
    pub source: String,
}

impl ActiveEmergencyInstance {
    pub fn new(emergency_id: impl Into<String>, activated_tick: u64, severity: f32) -> Self {
        Self {
            emergency_id: emergency_id.into(),
            activated_tick,
            severity: severity.clamp(0.0, 1.0),
            last_signal: severity.clamp(0.0, 1.0),
            manual_force: false,
            manual_suppress: false,
            acknowledged: false,
            source: String::new(),
        }
    }
}

/// Persistent emergency state (SA1 storage + SA8 continuity).
///
/// Legacy boolean flags remain for scene compatibility and are synced from `instances`.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct SettlementEmergencyState {
    /// Authoritative active emergency instances.
    #[serde(default)]
    pub instances: Vec<ActiveEmergencyInstance>,
    /// Legacy convenience flags (synced from instances; also migrate old scenes).
    #[serde(default)]
    pub starvation: bool,
    #[serde(default)]
    pub under_attack: bool,
    #[serde(default)]
    pub disease: bool,
    #[serde(default)]
    pub evacuation: bool,
}

impl Default for SettlementEmergencyState {
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            starvation: false,
            under_attack: false,
            disease: false,
            evacuation: false,
        }
    }
}

impl SettlementEmergencyState {
    pub fn any_active(&self) -> bool {
        !self.instances.is_empty()
            || self.starvation
            || self.under_attack
            || self.disease
            || self.evacuation
    }

    pub fn instance(&self, emergency_id: &str) -> Option<&ActiveEmergencyInstance> {
        self.instances
            .iter()
            .find(|i| i.emergency_id == emergency_id)
    }

    pub fn instance_mut(&mut self, emergency_id: &str) -> Option<&mut ActiveEmergencyInstance> {
        self.instances
            .iter_mut()
            .find(|i| i.emergency_id == emergency_id)
    }

    /// Sync legacy bools from instances. Call after mutate / load migrate.
    pub fn sync_legacy_flags(&mut self) {
        self.starvation = self.instances.iter().any(|i| i.emergency_id == "starvation");
        self.under_attack = self
            .instances
            .iter()
            .any(|i| i.emergency_id == "active_attack");
        self.disease = self.instances.iter().any(|i| i.emergency_id == "disease");
        self.evacuation = self
            .instances
            .iter()
            .any(|i| i.emergency_id == "evacuation");
    }

    /// After load: promote legacy bools into instances when instances empty.
    pub fn migrate_legacy_flags(&mut self, simulation_tick: u64) {
        if !self.instances.is_empty() {
            self.sync_legacy_flags();
            return;
        }
        let mut promoted = Vec::new();
        if self.starvation {
            promoted.push(ActiveEmergencyInstance::new("starvation", simulation_tick, 1.0));
        }
        if self.under_attack {
            promoted.push(ActiveEmergencyInstance::new(
                "active_attack",
                simulation_tick,
                1.0,
            ));
        }
        if self.disease {
            promoted.push(ActiveEmergencyInstance::new("disease", simulation_tick, 1.0));
        }
        if self.evacuation {
            promoted.push(ActiveEmergencyInstance::new("evacuation", simulation_tick, 1.0));
        }
        self.instances = promoted;
        self.sync_legacy_flags();
    }
}

/// Planner lifecycle bookkeeping. Does not perform evaluation.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct SettlementPlannerLifecycle {
    pub enabled: bool,
    pub paused: bool,
    pub last_evaluation_tick: u64,
    pub next_scheduled_evaluation_tick: u64,
    pub evaluation_interval_ticks: u64,
    /// Whether future planners may retain diagnostics after evaluation (config only).
    pub diagnostics_enabled: bool,
    /// Runtime invalidation flag. Never serialized — always dirty after load (rebuild principle).
    #[serde(skip, default = "default_planner_dirty")]
    pub dirty: bool,
}

fn default_planner_dirty() -> bool {
    true
}

impl Default for SettlementPlannerLifecycle {
    fn default() -> Self {
        Self {
            enabled: true,
            paused: false,
            last_evaluation_tick: 0,
            next_scheduled_evaluation_tick: 0,
            evaluation_interval_ticks: 60,
            diagnostics_enabled: true,
            dirty: true,
        }
    }
}

impl SettlementPlannerLifecycle {
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

/// Authoritative persistent settlement brain (SA1).
///
/// Answers: what about this settlement survives save/load?
/// Does **not** answer: what should this settlement do?
///
/// Derived analysis (need pressures, priorities, response graphs, planner caches, temporary
/// diagnostics) must never live here as authoritative fields.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct SettlementState {
    pub settlement_id: SettlementId,
    pub kind: SettlementKind,
    pub policies: SettlementPolicies,
    #[serde(default)]
    pub need_targets: Vec<NeedTarget>,
    #[serde(default)]
    pub modifiers: Vec<SettlementModifier>,
    #[serde(default)]
    pub emergencies: SettlementEmergencyState,
    #[serde(default)]
    pub planner: SettlementPlannerLifecycle,
    /// Opaque forward-compatible key/value seam for future SA phases.
    #[serde(default)]
    pub extension_seams: BTreeMap<String, String>,
}

impl SettlementState {
    /// Create default state for a settlement (identical structure for player and AI).
    pub fn new(settlement_id: SettlementId, kind: SettlementKind, player_controlled: bool) -> Self {
        let mut policies = SettlementPolicies::default();
        policies.player_controlled = player_controlled;
        Self {
            settlement_id,
            kind,
            policies,
            need_targets: default_need_targets_for_kind(kind),
            modifiers: Vec::new(),
            emergencies: SettlementEmergencyState::default(),
            planner: SettlementPlannerLifecycle::default(),
            extension_seams: BTreeMap::new(),
        }
    }

    pub fn mark_dirty(&mut self) {
        self.planner.mark_dirty();
    }
}

/// Authored default targets by kind. Defaults only — no behavior.
pub fn default_need_targets_for_kind(kind: SettlementKind) -> Vec<NeedTarget> {
    // Growth is always present at low weight (ADR-115 §6.3 seam).
    let growth = NeedTarget::new(NeedCategory::Growth, 1, 0.15);
    match kind {
        SettlementKind::Town | SettlementKind::Village | SettlementKind::Outpost => vec![
            NeedTarget::new(NeedCategory::Food, 100, 1.0),
            NeedTarget::new(NeedCategory::Water, 80, 0.9),
            NeedTarget::new(NeedCategory::Housing, 20, 0.7),
            NeedTarget::new(NeedCategory::Defense, 10, 0.5),
            growth,
        ],
        SettlementKind::Hive => vec![
            NeedTarget::new(NeedCategory::Food, 150, 1.0),
            NeedTarget::new(NeedCategory::Population, 50, 0.8),
            NeedTarget::new(NeedCategory::Defense, 20, 0.6),
            growth,
        ],
        SettlementKind::Pack | SettlementKind::Herd => vec![
            NeedTarget::new(NeedCategory::Food, 40, 1.0),
            NeedTarget::new(NeedCategory::Population, 12, 0.7),
            growth,
        ],
        SettlementKind::Camp => vec![
            NeedTarget::new(NeedCategory::Food, 30, 1.0),
            NeedTarget::new(NeedCategory::Defense, 15, 0.8),
            growth,
        ],
    }
}

/// Serializable envelope for scene / save persistence.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SettlementStateSaveState {
    #[serde(default)]
    pub states: BTreeMap<u64, SettlementState>,
}
