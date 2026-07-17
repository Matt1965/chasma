//! Starter building field requirements (ADR-104 TF4).

use super::definition::{BuildingFieldRequirementDefinition, BuildingFieldRequirementKind};
use crate::world::building::field_response::FieldResponseProfileId;
use crate::world::{BuildingDefinitionId, FootprintId, TerrainFieldId};

pub fn starter_requirements() -> Vec<BuildingFieldRequirementDefinition> {
    vec![
        iron_mine(),
        copper_mine(),
        stone_quarry(),
        prispod_farm(),
        well(),
    ]
}

fn iron_mine() -> BuildingFieldRequirementDefinition {
    requirement(
        "iron_mine",
        "iron",
        "iron_mine_monotonic",
        field_percent(30.0),
        field_percent(25.0),
        5_000,
        None,
        true,
        0,
    )
}

fn copper_mine() -> BuildingFieldRequirementDefinition {
    requirement(
        "copper_mine",
        "copper",
        "copper_mine_monotonic",
        field_percent(30.0),
        field_percent(25.0),
        5_000,
        None,
        true,
        0,
    )
}

fn stone_quarry() -> BuildingFieldRequirementDefinition {
    requirement(
        "stone_quarry",
        "stone",
        "stone_quarry_monotonic",
        field_percent(25.0),
        field_percent(20.0),
        4_000,
        Some(FootprintId::new("quarry_excavation")),
        true,
        0,
    )
}

fn prispod_farm() -> BuildingFieldRequirementDefinition {
    requirement(
        "prispod_farm",
        "water",
        "water_crop_preferred_range",
        field_percent(35.0),
        field_percent(30.0),
        8_000,
        Some(FootprintId::new("farm_cultivation")),
        true,
        0,
    )
}

fn well() -> BuildingFieldRequirementDefinition {
    requirement(
        "water_well",
        "water",
        "water_well_monotonic",
        field_percent(20.0),
        field_percent(15.0),
        6_000,
        Some(FootprintId::new("well_extraction")),
        true,
        0,
    )
}

fn requirement(
    building_id: &str,
    field_id: &str,
    profile_id: &str,
    minimum_average: u16,
    usable_threshold: u16,
    minimum_coverage_bp: u16,
    sampling_footprint_id: Option<FootprintId>,
    primary_overlay: bool,
    overlay_priority: u32,
) -> BuildingFieldRequirementDefinition {
    BuildingFieldRequirementDefinition {
        building_definition_id: BuildingDefinitionId::new(building_id),
        terrain_field_id: TerrainFieldId::new(field_id),
        requirement_kind: BuildingFieldRequirementKind::RequiredEfficiency,
        response_profile_id: FieldResponseProfileId::new(profile_id),
        minimum_average,
        minimum_usable_coverage_basis_points: minimum_coverage_bp,
        usable_value_threshold: usable_threshold,
        sampling_footprint_id,
        primary_overlay,
        overlay_priority,
        enabled: true,
    }
}

fn field_percent(percent: f32) -> u16 {
    crate::world::building::field_response::field_value_from_percent(percent)
}
