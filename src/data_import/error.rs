use std::path::PathBuf;

use crate::world::{
    AnimationProfileId, BuildingCategoryId, BuildingDefinitionId, DoodadDefinitionId,
    InventoryProfileId, ItemCategoryId, ItemDefinitionId, UnitDefinitionId, WeaponDefinitionId,
};

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
    DuplicateUnitId {
        id: UnitDefinitionId,
        first_row: usize,
        duplicate_row: usize,
    },
    DuplicateWeaponId {
        id: WeaponDefinitionId,
        first_row: usize,
        duplicate_row: usize,
    },
    DuplicateAnimationProfileId {
        id: AnimationProfileId,
        first_row: usize,
        duplicate_row: usize,
    },
    DuplicateBuildingId {
        id: BuildingDefinitionId,
        first_row: usize,
        duplicate_row: usize,
    },
    DuplicateBuildingCategoryId {
        id: BuildingCategoryId,
        first_row: usize,
        duplicate_row: usize,
    },
    DuplicateItemId {
        id: ItemDefinitionId,
        first_row: usize,
        duplicate_row: usize,
    },
    DuplicateItemCategoryId {
        id: ItemCategoryId,
        first_row: usize,
        duplicate_row: usize,
    },
    DuplicateInventoryProfileId {
        id: InventoryProfileId,
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
            Self::DuplicateUnitId {
                id,
                first_row,
                duplicate_row,
            } => write!(
                f,
                "duplicate Unit ID `{}` (rows {first_row} and {duplicate_row})",
                id.as_str()
            ),
            Self::DuplicateWeaponId {
                id,
                first_row,
                duplicate_row,
            } => write!(
                f,
                "duplicate Weapon ID `{}` (rows {first_row} and {duplicate_row})",
                id.as_str()
            ),
            Self::DuplicateAnimationProfileId {
                id,
                first_row,
                duplicate_row,
            } => write!(
                f,
                "duplicate Animation Profile ID `{}` (rows {first_row} and {duplicate_row})",
                id.as_str()
            ),
            Self::DuplicateBuildingId {
                id,
                first_row,
                duplicate_row,
            } => write!(
                f,
                "duplicate Building ID `{}` (rows {first_row} and {duplicate_row})",
                id.as_str()
            ),
            Self::DuplicateBuildingCategoryId {
                id,
                first_row,
                duplicate_row,
            } => write!(
                f,
                "duplicate Building Category ID `{}` (rows {first_row} and {duplicate_row})",
                id.as_str()
            ),
            Self::DuplicateItemId {
                id,
                first_row,
                duplicate_row,
            } => write!(
                f,
                "duplicate Item ID `{}` (rows {first_row} and {duplicate_row})",
                id.as_str()
            ),
            Self::DuplicateItemCategoryId {
                id,
                first_row,
                duplicate_row,
            } => write!(
                f,
                "duplicate Item Category ID `{}` (rows {first_row} and {duplicate_row})",
                id.as_str()
            ),
            Self::DuplicateInventoryProfileId {
                id,
                first_row,
                duplicate_row,
            } => write!(
                f,
                "duplicate Inventory Profile ID `{}` (rows {first_row} and {duplicate_row})",
                id.as_str()
            ),
            Self::NoValidRows => write!(f, "no valid rows after import"),
        }
    }
}

impl std::error::Error for DataImportError {}
