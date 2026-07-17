//! Authoritative building terrain assessment (ADR-104 TF4).

use bevy::prelude::*;

use super::error::TerrainAssessmentCatalogs;
use super::revision::hash_sample_cells;
use super::sample_cells::resolve_building_field_sample_cells;
use super::types::{
    BuildingFieldRequirementAssessment, BuildingTerrainAssessment, BuildingTerrainWarning,
    FieldTileRevisionEntry, RequirementAssessmentAvailability,
};
use crate::world::BasisPoints;
use crate::world::TerrainFieldId;
use crate::world::building::catalog::BuildingDefinitionId;
use crate::world::building::field_requirement::{
    BuildingFieldRequirementCatalog, BuildingFieldRequirementKind,
};
use crate::world::building::field_response::{
    EfficiencyBasisPoints, evaluate_field_response, field_value_to_percent_display,
};
use crate::world::building::placement::BuildingPlacement;
use crate::world::building::record::BuildingRecord;
use crate::world::terrain_field::{
    TerrainFieldAreaReport, field_sample_region_from_cells, sample_terrain_field_area,
    sample_terrain_field_at,
};
use crate::world::{ChunkLayout, WorldData, WorldPosition};

/// Assess terrain for a candidate placement (Build Mode preview and commit).
pub fn assess_building_terrain_at_placement(
    world: &WorldData,
    catalogs: &TerrainAssessmentCatalogs<'_>,
    building_definition_id: &BuildingDefinitionId,
    placement: BuildingPlacement,
    layout: ChunkLayout,
) -> BuildingTerrainAssessment {
    let Some(definition) = catalogs.buildings.get(building_definition_id) else {
        return empty_assessment(
            building_definition_id.clone(),
            catalogs.requirement_revision,
            catalogs.profile_revision,
        );
    };

    let requirements = catalogs
        .requirements
        .active_required_efficiency(building_definition_id);
    if requirements.is_empty() {
        return BuildingTerrainAssessment {
            building_definition_id: building_definition_id.clone(),
            per_requirement: Vec::new(),
            terrain_efficiency_basis_points: EfficiencyBasisPoints::ONE_HUNDRED_PERCENT,
            limiting_field: None,
            can_operate: true,
            sample_footprint_hash: 0,
            field_tile_revisions: Vec::new(),
            requirement_catalog_revision: catalogs.requirement_revision,
            profile_catalog_revision: catalogs.profile_revision,
            warnings: Vec::new(),
            stale: false,
        };
    }

    let mut per_requirement = Vec::with_capacity(requirements.len());
    let mut combined_cells_hash = 0u64;
    let mut all_tile_revisions = Vec::new();

    for requirement in requirements {
        let cells = match resolve_building_field_sample_cells(
            definition,
            requirement,
            &placement,
            catalogs.footprints,
            layout,
        ) {
            Ok(cells) => cells,
            Err(_) => {
                per_requirement.push(unavailable_requirement_assessment(requirement));
                continue;
            }
        };
        combined_cells_hash ^= hash_sample_cells(&cells);
        let region = field_sample_region_from_cells(cells);
        let area = sample_terrain_field_area(
            world,
            catalogs.fields,
            &requirement.terrain_field_id,
            &region,
            requirement.usable_value_threshold,
        );
        collect_tile_revisions(
            world,
            catalogs.fields,
            &requirement.terrain_field_id,
            &region,
            layout,
            &mut all_tile_revisions,
        );
        per_requirement.push(assess_requirement(requirement, catalogs, &area));
    }

    combine_assessment(
        building_definition_id.clone(),
        per_requirement,
        combined_cells_hash,
        dedupe_tile_revisions(all_tile_revisions),
        catalogs.requirement_revision,
        catalogs.profile_revision,
    )
}

/// Assess terrain for a placed building record.
pub fn assess_building_terrain(
    world: &WorldData,
    catalogs: &TerrainAssessmentCatalogs<'_>,
    record: &BuildingRecord,
    layout: ChunkLayout,
) -> BuildingTerrainAssessment {
    assess_building_terrain_at_placement(
        world,
        catalogs,
        &record.definition_id,
        record.placement,
        layout,
    )
}

fn assess_requirement(
    requirement: &crate::world::building::field_requirement::BuildingFieldRequirementDefinition,
    catalogs: &TerrainAssessmentCatalogs<'_>,
    area: &TerrainFieldAreaReport,
) -> BuildingFieldRequirementAssessment {
    let mut warnings = Vec::new();
    let availability =
        RequirementAssessmentAvailability::from_area(area.availability, area.requested_cells);

    if area.available_cells == 0 {
        warnings.push(BuildingTerrainWarning::DataUnavailable);
        return BuildingFieldRequirementAssessment {
            field_id: requirement.terrain_field_id.clone(),
            response_profile_id: requirement.response_profile_id.clone(),
            sample_count: area.requested_cells,
            unavailable_sample_count: area.unavailable_cells,
            average_value: None,
            minimum_value: None,
            maximum_value: None,
            usable_coverage_basis_points: BasisPoints::ZERO,
            response_efficiency_basis_points: EfficiencyBasisPoints::ZERO,
            average_requirement_met: false,
            coverage_requirement_met: false,
            can_operate: false,
            availability,
            warnings,
        };
    }

    let average = area.average.unwrap_or(0);
    let average_requirement_met = average >= requirement.minimum_average;
    let coverage_requirement_met =
        area.usable_coverage.value() >= requirement.minimum_usable_coverage_basis_points;

    if !average_requirement_met {
        warnings.push(BuildingTerrainWarning::AverageBelowMinimum {
            field_id: requirement.terrain_field_id.clone(),
        });
    }
    if !coverage_requirement_met {
        warnings.push(BuildingTerrainWarning::CoverageBelowMinimum {
            field_id: requirement.terrain_field_id.clone(),
        });
    }

    let profile = catalogs
        .profiles
        .get(&requirement.response_profile_id)
        .expect("validated requirement references profile");
    let response_efficiency =
        evaluate_field_response(profile, average).unwrap_or(EfficiencyBasisPoints::ZERO);

    if response_efficiency.value() == 0 {
        warnings.push(BuildingTerrainWarning::PlacementAllowedZeroOutput);
    } else if response_efficiency.value() < EfficiencyBasisPoints::ONE_HUNDRED_PERCENT.value() {
        warnings.push(BuildingTerrainWarning::PlacementAllowedLowEfficiency);
    }

    let can_operate = availability == RequirementAssessmentAvailability::Available
        && average_requirement_met
        && coverage_requirement_met
        && response_efficiency.value() > 0;

    if !can_operate {
        warnings.push(BuildingTerrainWarning::CannotOperate);
    }

    BuildingFieldRequirementAssessment {
        field_id: requirement.terrain_field_id.clone(),
        response_profile_id: requirement.response_profile_id.clone(),
        sample_count: area.requested_cells,
        unavailable_sample_count: area.unavailable_cells,
        average_value: area.average,
        minimum_value: area.minimum,
        maximum_value: area.maximum,
        usable_coverage_basis_points: area.usable_coverage,
        response_efficiency_basis_points: response_efficiency,
        average_requirement_met,
        coverage_requirement_met,
        can_operate,
        availability,
        warnings,
    }
}

fn unavailable_requirement_assessment(
    requirement: &crate::world::building::field_requirement::BuildingFieldRequirementDefinition,
) -> BuildingFieldRequirementAssessment {
    BuildingFieldRequirementAssessment {
        field_id: requirement.terrain_field_id.clone(),
        response_profile_id: requirement.response_profile_id.clone(),
        sample_count: 0,
        unavailable_sample_count: 0,
        average_value: None,
        minimum_value: None,
        maximum_value: None,
        usable_coverage_basis_points: BasisPoints::ZERO,
        response_efficiency_basis_points: EfficiencyBasisPoints::ZERO,
        average_requirement_met: false,
        coverage_requirement_met: false,
        can_operate: false,
        availability: RequirementAssessmentAvailability::Unavailable,
        warnings: vec![BuildingTerrainWarning::DataUnavailable],
    }
}

fn combine_assessment(
    building_definition_id: BuildingDefinitionId,
    per_requirement: Vec<BuildingFieldRequirementAssessment>,
    sample_footprint_hash: u64,
    field_tile_revisions: Vec<FieldTileRevisionEntry>,
    requirement_catalog_revision: u64,
    profile_catalog_revision: u64,
) -> BuildingTerrainAssessment {
    let mut warnings = Vec::new();
    for assessment in &per_requirement {
        warnings.extend(assessment.warnings.clone());
    }

    let terrain_efficiency_basis_points = per_requirement
        .iter()
        .map(|req| req.response_efficiency_basis_points)
        .min_by_key(|eff| eff.value())
        .unwrap_or(EfficiencyBasisPoints::ONE_HUNDRED_PERCENT);

    let limiting_field = select_limiting_field(&per_requirement);
    let can_operate =
        !per_requirement.is_empty() && per_requirement.iter().all(|req| req.can_operate);

    BuildingTerrainAssessment {
        building_definition_id,
        per_requirement,
        terrain_efficiency_basis_points,
        limiting_field,
        can_operate,
        sample_footprint_hash,
        field_tile_revisions,
        requirement_catalog_revision,
        profile_catalog_revision,
        warnings,
        stale: false,
    }
}

fn select_limiting_field(
    per_requirement: &[BuildingFieldRequirementAssessment],
) -> Option<TerrainFieldId> {
    if per_requirement.is_empty() {
        return None;
    }
    let mut ranked: Vec<&BuildingFieldRequirementAssessment> = per_requirement.iter().collect();
    ranked.sort_by(|left, right| {
        let left_key = limiting_sort_key(left);
        let right_key = limiting_sort_key(right);
        left_key
            .cmp(&right_key)
            .then_with(|| left.field_id.cmp(&right.field_id))
    });
    ranked.first().map(|req| req.field_id.clone())
}

fn limiting_sort_key(assessment: &BuildingFieldRequirementAssessment) -> (u8, u32, u16) {
    let failure_rank = if assessment.availability != RequirementAssessmentAvailability::Available {
        0
    } else if !assessment.coverage_requirement_met {
        1
    } else if !assessment.average_requirement_met {
        2
    } else {
        3
    };
    (
        failure_rank,
        assessment.response_efficiency_basis_points.value(),
        assessment.usable_coverage_basis_points.value(),
    )
}

fn collect_tile_revisions(
    world: &WorldData,
    field_catalog: &crate::world::TerrainFieldCatalog,
    field_id: &TerrainFieldId,
    region: &crate::world::terrain_field::FieldSampleRegion,
    layout: ChunkLayout,
    out: &mut Vec<FieldTileRevisionEntry>,
) {
    for cell in region.cells() {
        let center = cell.center_global();
        let position = WorldPosition::from_global(Vec3::new(center.x, 0.0, center.y), layout);
        let sample = sample_terrain_field_at(world, field_catalog, field_id, position);
        if let (Some(chunk), Some(revision)) = (sample.chunk, sample.tile_revision) {
            out.push(FieldTileRevisionEntry {
                field_id: field_id.clone(),
                chunk,
                tile_revision: revision,
            });
        }
    }
}

fn dedupe_tile_revisions(mut entries: Vec<FieldTileRevisionEntry>) -> Vec<FieldTileRevisionEntry> {
    entries.sort_by(|a, b| {
        (a.field_id.as_str(), a.chunk.x, a.chunk.z).cmp(&(
            b.field_id.as_str(),
            b.chunk.x,
            b.chunk.z,
        ))
    });
    entries.dedup_by(|a, b| {
        a.field_id == b.field_id && a.chunk == b.chunk && a.tile_revision == b.tile_revision
    });
    entries
}

fn empty_assessment(
    building_definition_id: BuildingDefinitionId,
    requirement_catalog_revision: u64,
    profile_catalog_revision: u64,
) -> BuildingTerrainAssessment {
    BuildingTerrainAssessment {
        building_definition_id,
        per_requirement: Vec::new(),
        terrain_efficiency_basis_points: EfficiencyBasisPoints::ZERO,
        limiting_field: None,
        can_operate: false,
        sample_footprint_hash: 0,
        field_tile_revisions: Vec::new(),
        requirement_catalog_revision,
        profile_catalog_revision,
        warnings: vec![BuildingTerrainWarning::DataUnavailable],
        stale: true,
    }
}

/// Format average field value for UI — distinguishes unknown from zero.
pub fn format_field_average_display(average: Option<u16>) -> String {
    match average {
        Some(value) => format!("{:.0}%", field_value_to_percent_display(value)),
        None => "Unknown".to_string(),
    }
}

/// Format coverage for UI.
pub fn format_coverage_display(coverage: BasisPoints) -> String {
    format!("{:.0}%", coverage.as_percent_display())
}

/// Format terrain efficiency for UI (may exceed 100%).
pub fn format_efficiency_display(efficiency: EfficiencyBasisPoints) -> String {
    format!("{:.0}%", efficiency.as_percent_display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::catalog::BuildingCatalog;
    use crate::world::building::field_requirement::BuildingFieldRequirementCatalog;
    use crate::world::building::field_response::FieldResponseProfileCatalog;
    use crate::world::terrain_field::{TerrainFieldCatalog, TerrainFieldId};
    use crate::world::{
        BuildingCategoryCatalog, ChunkCoord, ChunkExtent, FootprintCatalog, LocalPosition,
        WorldConfig,
    };

    fn test_catalogs() -> (WorldData, TerrainAssessmentCatalogs<'static>) {
        let layout = WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        });
        crate::world::bootstrap_constant_field(
            world.terrain_fields_mut(),
            TerrainFieldId::new("iron"),
            ChunkCoord::new(0, 0),
            crate::world::building::field_response::field_value_from_percent(94.0),
        );
        let field_catalog = TerrainFieldCatalog::default();
        let profile_catalog = FieldResponseProfileCatalog::default();
        let requirement_catalog = BuildingFieldRequirementCatalog::default();
        let categories = BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_buildings(), &categories).unwrap();
        let footprint_catalog = FootprintCatalog::default();
        let catalogs = TerrainAssessmentCatalogs {
            buildings: Box::leak(Box::new(building_catalog)),
            requirements: Box::leak(Box::new(requirement_catalog)),
            profiles: Box::leak(Box::new(profile_catalog)),
            fields: Box::leak(Box::new(field_catalog)),
            footprints: Box::leak(Box::new(footprint_catalog)),
            requirement_revision: 0,
            profile_revision: 0,
        };
        (world, catalogs)
    }

    fn starter_buildings() -> Vec<crate::world::building::catalog::BuildingDefinition> {
        use crate::world::building::catalog::BuildingDefinition;
        use crate::world::building::catalog::BuildingDefinitionId;
        use crate::world::building::catalog::BuildingRenderKey;
        use crate::world::building::category::BuildingCategoryId;
        use crate::world::building::footprint::FootprintSpec;
        vec![BuildingDefinition::new(
            BuildingDefinitionId::new("iron_mine"),
            "Iron Mine",
            BuildingCategoryId::new("production"),
            BuildingRenderKey::reserved("smelter"),
            BuildingRenderKey::reserved("smelter_collision"),
            400,
            90.0,
            FootprintSpec::Circle { radius_meters: 2.5 },
            30.0,
            true,
        )]
    }

    #[test]
    fn rich_iron_mine_can_exceed_one_hundred_percent() {
        let (world, catalogs) = test_catalogs();
        let layout = WorldConfig::default().chunk_layout();
        let placement = BuildingPlacement::new(
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(64.0, 0.0, 64.0)),
            ),
            Quat::IDENTITY,
        );
        let assessment = assess_building_terrain_at_placement(
            &world,
            &catalogs,
            &BuildingDefinitionId::new("iron_mine"),
            placement,
            layout,
        );
        assert!(assessment.terrain_efficiency_basis_points.value() > 10_000);
        assert!(assessment.can_operate);
    }
}
