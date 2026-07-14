//! Inventory profile validation (ADR-087 I1).

use super::profile::InventoryProfileDefinition;
use super::profile_id::InventoryProfileId;

/// Maximum grid width/height for inventory profiles (safety bound).
pub const MAX_INVENTORY_GRID_DIMENSION: u8 = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryProfileValidationError {
    MissingInventoryProfileId {
        row_number: Option<usize>,
    },
    InvalidInventoryProfile {
        row_number: Option<usize>,
        profile_id: InventoryProfileId,
        message: String,
    },
    InvalidStackCap {
        row_number: Option<usize>,
        profile_id: InventoryProfileId,
        cap: u32,
    },
    EmptyDisplayName {
        row_number: Option<usize>,
        profile_id: InventoryProfileId,
    },
}

impl std::fmt::Display for InventoryProfileValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingInventoryProfileId { row_number } => {
                write!(f, "missing inventory profile id")?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
            Self::InvalidInventoryProfile {
                row_number,
                profile_id,
                message,
            } => {
                write!(f, "inventory profile `{}`: {message}", profile_id.as_str())?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
            Self::InvalidStackCap {
                row_number,
                profile_id,
                cap,
            } => {
                write!(
                    f,
                    "inventory profile `{}` has invalid global stack cap {cap}",
                    profile_id.as_str()
                )?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
            Self::EmptyDisplayName {
                row_number,
                profile_id,
            } => {
                write!(
                    f,
                    "inventory profile `{}` has empty display name",
                    profile_id.as_str()
                )?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for InventoryProfileValidationError {}

pub fn validate_inventory_profile(
    profile: &InventoryProfileDefinition,
    row_number: Option<usize>,
) -> Result<(), InventoryProfileValidationError> {
    let profile_id = profile.id.clone();

    if profile.id.as_str().trim().is_empty() {
        return Err(InventoryProfileValidationError::MissingInventoryProfileId { row_number });
    }
    if profile.display_name.trim().is_empty() {
        return Err(InventoryProfileValidationError::EmptyDisplayName {
            row_number,
            profile_id,
        });
    }

    if profile.grid_width == 0
        || profile.grid_height == 0
        || profile.grid_width > MAX_INVENTORY_GRID_DIMENSION
        || profile.grid_height > MAX_INVENTORY_GRID_DIMENSION
    {
        return Err(InventoryProfileValidationError::InvalidInventoryProfile {
            row_number,
            profile_id,
            message: format!(
                "invalid grid dimensions {}x{}",
                profile.grid_width, profile.grid_height
            ),
        });
    }

    if let Some(cap) = profile.global_stack_cap {
        if cap < 1 {
            return Err(InventoryProfileValidationError::InvalidStackCap {
                row_number,
                profile_id,
                cap,
            });
        }
    }

    Ok(())
}

/// Whether reference weight is soft metadata only (I1 has no hard mass rejection).
pub fn reference_weight_is_soft_encumbrance(profile: &InventoryProfileDefinition) -> bool {
    profile.reference_weight_grams.is_some()
}

#[cfg(test)]
mod tests {
    use super::super::profile::InventoryProfileDefinition;
    use super::*;

    fn valid_profile() -> InventoryProfileDefinition {
        InventoryProfileDefinition::new(
            InventoryProfileId::new("unit_backpack_standard"),
            "Standard Backpack",
            6,
            6,
            true,
        )
        .with_reference_weight_grams(15_000)
    }

    #[test]
    fn valid_grid_profile_passes() {
        validate_inventory_profile(&valid_profile(), None).expect("valid");
    }

    #[test]
    fn zero_dimensions_rejected() {
        let mut profile = valid_profile();
        profile.grid_width = 0;
        assert!(matches!(
            validate_inventory_profile(&profile, None),
            Err(InventoryProfileValidationError::InvalidInventoryProfile { .. })
        ));
    }

    #[test]
    fn invalid_stack_cap_rejected() {
        let profile = valid_profile().with_global_stack_cap(0);
        assert!(matches!(
            validate_inventory_profile(&profile, None),
            Err(InventoryProfileValidationError::InvalidStackCap { .. })
        ));
    }

    #[test]
    fn reference_weight_does_not_become_hard_capacity() {
        let profile = valid_profile();
        assert!(reference_weight_is_soft_encumbrance(&profile));
    }
}
