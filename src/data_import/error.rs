use std::path::PathBuf;

use crate::world::DoodadDefinitionId;

/// Errors that abort the entire import (sheet layout, I/O, zero valid rows).
#[derive(Debug, Clone, PartialEq)]
pub enum DataImportError {
    Io {
        path: PathBuf,
        message: String,
    },
    WorkbookOpen(String),
    SheetNotFound {
        sheet: String,
    },
    MissingRequiredColumn {
        column: String,
    },
    DuplicateName {
        name: DoodadDefinitionId,
        first_row: usize,
        duplicate_row: usize,
    },
    NoValidRows,
}

/// A single data row failed validation and was skipped.
#[derive(Debug, Clone, PartialEq)]
pub struct RowImportError {
    pub row_number: usize,
    pub message: String,
}

impl std::fmt::Display for DataImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "read {}: {message}", path.display()),
            Self::WorkbookOpen(message) => write!(f, "open workbook: {message}"),
            Self::SheetNotFound { sheet } => write!(f, "sheet `{sheet}` not found"),
            Self::MissingRequiredColumn { column } => {
                write!(f, "missing required column `{column}`")
            }
            Self::DuplicateName {
                name,
                first_row,
                duplicate_row,
            } => write!(
                f,
                "duplicate Name `{}` (rows {first_row} and {duplicate_row})",
                name.as_str()
            ),
            Self::NoValidRows => write!(f, "no valid doodad rows after import"),
        }
    }
}

impl std::error::Error for DataImportError {}
