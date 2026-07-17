//! Bottom-left building info when a building is selected (ADR-082 B5, ADR-104 TF4, ADR-105 TF5).

use bevy::prelude::*;

use crate::world::{
    BuildingCatalog, BuildingFieldRequirementCatalog, BuildingFieldRequirementCatalogRevision,
    BuildingOperationParams, BuildingOperationStore, BuildingTerrainAssessmentStore,
    FieldResponseProfileCatalog, FieldResponseProfileCatalogRevision, FootprintCatalog,
    OperationalLimitingFactor, PRODUCTION_PROGRESS_ONE_UNIT, TerrainFieldCatalog, WorldData,
    building_operational_efficiency, field_value_to_percent_display, format_efficiency_display,
    is_building_operational,
};

use super::building_selection::GameplayBuildingSelection;

#[derive(Component, Debug)]
pub struct SelectedBuildingPanelRoot;

#[derive(Component, Debug)]
pub(crate) struct SelectedBuildingPanelText;

pub fn spawn_selected_building_panel(parent: &mut ChildSpawnerCommands<'_>) {
    parent.spawn((
        SelectedBuildingPanelRoot,
        SelectedBuildingPanelText,
        Text::new(""),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgba(0.85, 0.9, 0.95, 1.0)),
        Node {
            display: Display::None,
            max_width: Val::Px(260.0),
            ..default()
        },
    ));
}

pub fn sync_selected_building_panel(
    selection: Res<GameplayBuildingSelection>,
    world: Res<WorldData>,
    catalog: Res<BuildingCatalog>,
    field_catalog: Res<TerrainFieldCatalog>,
    requirements: Res<BuildingFieldRequirementCatalog>,
    profile_catalog: Res<FieldResponseProfileCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
    requirement_revision: Res<BuildingFieldRequirementCatalogRevision>,
    profile_revision: Res<FieldResponseProfileCatalogRevision>,
    mut assessments: ResMut<BuildingTerrainAssessmentStore>,
    operation_store: Res<BuildingOperationStore>,
    mut text: Query<(&mut Text, &mut Node), With<SelectedBuildingPanelText>>,
) {
    let Ok((mut label, mut node)) = text.single_mut() else {
        return;
    };

    let Some(building_id) = selection.building_id else {
        node.display = Display::None;
        return;
    };

    let Some(record) = world.get_building(building_id) else {
        node.display = Display::None;
        return;
    };
    let display_name = catalog
        .get(&record.definition_id)
        .map(|def| def.display_name.as_str())
        .unwrap_or(record.definition_id.as_str());

    let mut body = format!(
        "Building: {}\nState: {} ({:.0}%)\nHP: {}/{}\n{}",
        display_name,
        record.lifecycle_state.label(),
        record.construction.progress_0_1 * 100.0,
        record.vitals.current_hp,
        record.vitals.max_hp,
        if is_building_operational(record) {
            "Operational"
        } else {
            "Not operational"
        },
    );

    let has_field_requirements = !requirements
        .active_required_efficiency(&record.definition_id)
        .is_empty();

    if has_field_requirements {
        body.push_str("\n\nTerrain Suitability");
        if let Some(assessment) = assessments.get(building_id) {
            for requirement in &assessment.per_requirement {
                let field_name = field_catalog
                    .get(&requirement.field_id)
                    .map(|field| field.display_name.as_str())
                    .unwrap_or(requirement.field_id.as_str());
                let average = requirement
                    .average_value
                    .map(field_value_to_percent_display)
                    .map(|value| format!("{value:.0}%"))
                    .unwrap_or_else(|| "Unknown".to_string());
                body.push_str(&format!(
                    "\n{field_name}: {average} (min {:.0}%)",
                    field_value_to_percent_display(
                        requirements
                            .lookup(&record.definition_id, &requirement.field_id)
                            .map(|req| req.minimum_average)
                            .unwrap_or(0),
                    )
                ));
                body.push_str(&format!(
                    "\n  Coverage: {:.0}%",
                    requirement
                        .usable_coverage_basis_points
                        .as_percent_display()
                ));
                body.push_str(&format!(
                    "\n  Efficiency: {}",
                    format_efficiency_display(requirement.response_efficiency_basis_points)
                ));
            }
            body.push_str(&format!(
                "\nTerrain Efficiency: {}",
                format_efficiency_display(assessment.terrain_efficiency_basis_points)
            ));
            if let Some(limiting) = &assessment.limiting_field {
                body.push_str(&format!("\nLimiting Field: {}", limiting.as_str()));
            }
            body.push_str(&format!("\nCan Operate: {}", assessment.status_label()));
            if assessment.stale {
                body.push_str("\nAssessment: stale");
            }
        } else {
            body.push_str("\nAssessment unavailable");
        }
    }

    if is_building_operational(record) {
        let mut operation_store_scratch = BuildingOperationStore::default();
        let mut operation = BuildingOperationParams {
            field_catalog: &field_catalog,
            requirement_catalog: &requirements,
            profile_catalog: &profile_catalog,
            footprint_catalog: &footprint_catalog,
            requirement_revision: requirement_revision.0,
            profile_revision: profile_revision.0,
            assessment_store: &mut assessments,
            operation_store: &mut operation_store_scratch,
        };
        let mut efficiency_ctx = operation.efficiency_context(&world, &catalog);
        if let Ok(report) = building_operational_efficiency(&mut efficiency_ctx, building_id) {
            body.push_str("\n\nOperational Efficiency");
            body.push_str(&format!(
                "\nTerrain Output: {}",
                format_efficiency_display(report.terrain_efficiency_basis_points)
            ));
            body.push_str(&format!(
                "\nFinal Output Rate: {}",
                format_efficiency_display(report.final_output_efficiency_basis_points)
            ));
            if report.limiting_factor != OperationalLimitingFactor::None {
                body.push_str(&format!(
                    "\nLimiting Factor: {}",
                    report.limiting_factor.label()
                ));
            }
            body.push_str(&format!("\nCan Produce: {}", report.can_operate));
        }
        if let Some(state) = operation_store.get(building_id) {
            let progress_pct =
                state.progress.value() as f32 / PRODUCTION_PROGRESS_ONE_UNIT as f32 * 100.0;
            body.push_str(&format!("\nOperation Progress: {progress_pct:.1}%"));
            body.push_str(&format!("\nCompletions: {}", state.completion_count));
        }
    }

    node.display = Display::Flex;
    **label = body;
}
