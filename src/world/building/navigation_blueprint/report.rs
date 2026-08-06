//! Generation report for navigation blueprint pipeline (NV1.2).

use super::id::BuildingNavigationBlueprintId;

/// Mesh-slicing geometry diagnostics (client/tooling; not blueprint schema).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeometryGenerationDiagnostics {
    pub source_triangle_count: usize,
    pub walkable_triangle_count: usize,
    pub steep_triangle_discarded: usize,
    pub floor_cluster_count: usize,
    pub connected_component_count: usize,
    pub candidate_region_count: usize,
    pub candidate_connection_count: usize,
    pub regions_discarded: usize,
    pub used_collision_mesh: bool,
    pub used_render_fallback: bool,
    pub convex_hull_fallback_count: usize,
    pub multiple_boundary_loops: usize,
    pub ambiguous_opening_count: usize,
}

/// Compact entrance-generation diagnostics (client/tooling; not blueprint schema).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntranceGenerationDiagnostics {
    pub entrances_generated: usize,
    pub explicit_markers: usize,
    pub synthesized_entrances: usize,
    pub deduplicated_candidates: usize,
    pub candidate_details: Vec<String>,
}

/// One building blueprint generation outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct NavigationBlueprintGenerationReport {
    pub building_id: String,
    pub blueprint_id: BuildingNavigationBlueprintId,
    pub status: NavigationBlueprintGenerationStatus,
    /// Mesh geometry used for slicing (`occupancy_collision` or visible GLB fallback).
    pub mesh_source_label: Option<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    /// Entrance candidate counts from the last generate pass (tooling only).
    pub entrance_diagnostics: EntranceGenerationDiagnostics,
    /// Region/connection extraction stats from the last generate pass (tooling only).
    pub geometry_diagnostics: GeometryGenerationDiagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationBlueprintGenerationStatus {
    Generated,
    Cached,
    Skipped,
    Failed,
}

/// Write aggregated generation reports to markdown (mirrors asset sizing report).
pub fn export_generation_reports_markdown(
    path: &std::path::Path,
    reports: &[NavigationBlueprintGenerationReport],
) -> std::io::Result<()> {
    use std::io::Write;

    let mut file = std::fs::File::create(path)?;
    writeln!(file, "# Navigation Blueprint Generation Report")?;
    writeln!(file)?;
    writeln!(file, "Generated {} entries (NV1.2).", reports.len())?;
    writeln!(file)?;
    writeln!(
        file,
        "| Building | Blueprint | Status | Warnings | Errors |"
    )?;
    writeln!(
        file,
        "|----------|-----------|--------|----------|--------|"
    )?;
    for report in reports {
        writeln!(
            file,
            "| {} | {} | {:?} | {} | {} |",
            report.building_id,
            report.blueprint_id,
            report.status,
            report.warnings.len(),
            report.errors.len()
        )?;
        for warning in &report.warnings {
            writeln!(file, "| | | | ⚠ {warning} | |")?;
        }
        for error in &report.errors {
            writeln!(file, "| | | | | ✗ {error} |")?;
        }
    }
    Ok(())
}
