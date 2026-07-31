//! Generation report for navigation blueprint pipeline (NV1.2).

use super::id::BuildingNavigationBlueprintId;

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
