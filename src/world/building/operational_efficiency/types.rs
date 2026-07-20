use bevy::prelude::*;

use crate::world::TerrainFieldId;
use crate::world::building::field_response::EfficiencyBasisPoints;

/// Why a Building cannot produce at full (or any) output rate (ADR-105 TF5).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum OperationalLimitingFactor {
    None,
    BuildingIncomplete,
    BuildingDestroyed,
    BuildingDisabled,
    TerrainFieldUnavailable(TerrainFieldId),
    TerrainAverageBelowMinimum(TerrainFieldId),
    TerrainCoverageBelowMinimum(TerrainFieldId),
    TerrainResponseZero(TerrainFieldId),
    MissingTerrainAssessment,
    StaleTerrainAssessment,
    /// Required recipe input unavailable in bound inventory (EP5).
    MissingInput,
    /// Required input present but reserved for hauling (EP8).
    InputReserved,
    /// Output destination cannot accept produced items (EP5).
    OutputBlocked,
    /// Bound inventory record missing at runtime (EP5).
    MissingInventory,
    NoWorker,
    NoPower,
    /// Policy explicitly paused production (EP2).
    Paused,
    /// No operation selected or operation reference invalid (EP2).
    InvalidOperation,
    /// Building record missing while production state exists (EP2 validation).
    MissingBuilding,
    /// Operation references missing or incompatible inventory bindings (EP4).
    InvalidInventoryBinding,
}

impl OperationalLimitingFactor {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::BuildingIncomplete => "Building incomplete",
            Self::BuildingDestroyed => "Building destroyed",
            Self::BuildingDisabled => "Building disabled",
            Self::TerrainFieldUnavailable(_) => "Terrain field data unavailable",
            Self::TerrainAverageBelowMinimum(_) => "Terrain average below minimum",
            Self::TerrainCoverageBelowMinimum(_) => "Terrain coverage below minimum",
            Self::TerrainResponseZero(_) => "Terrain response efficiency is zero",
            Self::MissingTerrainAssessment => "Terrain assessment missing",
            Self::StaleTerrainAssessment => "Terrain assessment stale",
            Self::MissingInput => "Missing input",
            Self::InputReserved => "Input reserved",
            Self::OutputBlocked => "Output full",
            Self::MissingInventory => "Missing inventory",
            Self::NoWorker => "No worker assigned",
            Self::NoPower => "No power",
            Self::Paused => "Production paused",
            Self::InvalidOperation => "No valid operation selected",
            Self::MissingBuilding => "Building missing",
            Self::InvalidInventoryBinding => "Invalid inventory binding",
        }
    }
}

/// Authoritative operational output-efficiency query result (ADR-105 TF5).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct OperationalEfficiencyReport {
    pub building_id: crate::world::BuildingId,
    pub can_operate: bool,
    pub terrain_efficiency_basis_points: EfficiencyBasisPoints,
    pub worker_efficiency_basis_points: EfficiencyBasisPoints,
    pub condition_efficiency_basis_points: EfficiencyBasisPoints,
    pub other_efficiency_basis_points: EfficiencyBasisPoints,
    pub final_output_efficiency_basis_points: EfficiencyBasisPoints,
    pub limiting_factor: OperationalLimitingFactor,
    pub reasons: Vec<OperationalLimitingFactor>,
    pub assessment_revision: u64,
}

impl OperationalEfficiencyReport {
    pub fn output_rate_percent_display(&self) -> f32 {
        self.final_output_efficiency_basis_points
            .as_percent_display()
    }
}
