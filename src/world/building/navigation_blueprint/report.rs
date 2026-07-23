//! Generation report for navigation blueprint pipeline (NV1.2).

use super::id::BuildingNavigationBlueprintId;

/// One building blueprint generation outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationBlueprintGenerationReport {
    pub building_id: String,
    pub blueprint_id: BuildingNavigationBlueprintId,
    pub status: NavigationBlueprintGenerationStatus,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
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
    writeln!(
        file,
        "Generated {} entries (NV1.2).",
        reports.len()
    )?;
    writeln!(file)?;
    writeln!(file, "| Building | Blueprint | Status | Warnings | Errors |")?;
    writeln!(file, "|----------|-----------|--------|----------|--------|")?;
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
