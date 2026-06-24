//! Paths for offline design workbook import (dev / data-import).

use std::path::PathBuf;

/// Authoritative design workbook at the repo root (`Chasma Design.xlsx`).
pub const DEV_DESIGN_WORKBOOK: &str = "Chasma Design.xlsx";

/// Absolute path to [`DEV_DESIGN_WORKBOOK`] from the crate manifest directory.
pub fn dev_design_workbook_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEV_DESIGN_WORKBOOK)
}
