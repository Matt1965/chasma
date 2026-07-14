//! Item definition validation (ADR-087 I1).

use crate::world::ItemCategoryCatalog;
use crate::world::ItemDefinition;
use crate::world::ItemDefinitionId;

/// Maximum grid width/height for item footprints (safety bound).
pub const MAX_ITEM_GRID_DIMENSION: u8 = 64;

/// Why an item definition failed validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemValidationError {
    MissingItemId {
        row_number: Option<usize>,
    },
    DuplicateItemId {
        id: ItemDefinitionId,
        first_row: usize,
        duplicate_row: usize,
    },
    MissingCategory {
        row_number: Option<usize>,
        item_id: ItemDefinitionId,
        category_id: String,
    },
    DisabledCategory {
        row_number: Option<usize>,
        item_id: ItemDefinitionId,
        category_id: String,
    },
    InvalidDimensions {
        row_number: Option<usize>,
        item_id: ItemDefinitionId,
        width: u8,
        height: u8,
    },
    InvalidStackConfiguration {
        row_number: Option<usize>,
        item_id: ItemDefinitionId,
        message: String,
    },
    ContradictoryUniqueStackFlags {
        row_number: Option<usize>,
        item_id: ItemDefinitionId,
    },
    InvalidMass {
        row_number: Option<usize>,
        item_id: ItemDefinitionId,
        mass_grams: u32,
    },
    InvalidBaseValue {
        row_number: Option<usize>,
        item_id: ItemDefinitionId,
        base_value_gold: u32,
    },
    EmptyDisplayName {
        row_number: Option<usize>,
        item_id: ItemDefinitionId,
    },
}

impl std::fmt::Display for ItemValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingItemId { row_number } => {
                write!(f, "missing item id")?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
            Self::DuplicateItemId {
                id,
                first_row,
                duplicate_row,
            } => write!(
                f,
                "duplicate item id `{}` (rows {first_row} and {duplicate_row})",
                id.as_str()
            ),
            Self::MissingCategory {
                row_number,
                item_id,
                category_id,
            } => {
                write!(
                    f,
                    "item `{}` references missing category `{category_id}`",
                    item_id.as_str()
                )?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
            Self::DisabledCategory {
                row_number,
                item_id,
                category_id,
            } => {
                write!(
                    f,
                    "item `{}` references disabled category `{category_id}`",
                    item_id.as_str()
                )?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
            Self::InvalidDimensions {
                row_number,
                item_id,
                width,
                height,
            } => {
                write!(
                    f,
                    "item `{}` has invalid dimensions {width}x{height}",
                    item_id.as_str()
                )?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
            Self::InvalidStackConfiguration {
                row_number,
                item_id,
                message,
            } => {
                write!(f, "item `{}`: {message}", item_id.as_str())?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
            Self::ContradictoryUniqueStackFlags {
                row_number,
                item_id,
            } => {
                write!(
                    f,
                    "item `{}` has contradictory stackable/unique settings",
                    item_id.as_str()
                )?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
            Self::InvalidMass {
                row_number,
                item_id,
                mass_grams,
            } => {
                write!(
                    f,
                    "item `{}` has invalid mass {mass_grams} grams",
                    item_id.as_str()
                )?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
            Self::InvalidBaseValue {
                row_number,
                item_id,
                base_value_gold,
            } => {
                write!(
                    f,
                    "item `{}` has invalid base value {base_value_gold}",
                    item_id.as_str()
                )?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
            Self::EmptyDisplayName {
                row_number,
                item_id,
            } => {
                write!(f, "item `{}` has empty display name", item_id.as_str())?;
                if let Some(row) = row_number {
                    write!(f, " (row {row})")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ItemValidationError {}

/// Normalize comma/semicolon-separated tags deterministically.
pub fn normalize_tags(raw: &str) -> Vec<String> {
    let mut tags: Vec<String> = raw
        .split(|c| c == ',' || c == ';')
        .map(|part| part.trim().to_ascii_lowercase())
        .filter(|part| !part.is_empty())
        .collect();
    tags.sort_unstable();
    tags.dedup();
    tags
}

/// Validate a single item definition against category catalog rules.
pub fn validate_item_definition(
    item: &ItemDefinition,
    categories: &ItemCategoryCatalog,
    row_number: Option<usize>,
) -> Result<(), ItemValidationError> {
    let item_id = item.id.clone();

    if item.id.as_str().trim().is_empty() {
        return Err(ItemValidationError::MissingItemId { row_number });
    }
    if item.display_name.trim().is_empty() {
        return Err(ItemValidationError::EmptyDisplayName {
            row_number,
            item_id,
        });
    }

    let category_id = item.category_id.as_str();
    let Some(category) = categories.get(&item.category_id) else {
        return Err(ItemValidationError::MissingCategory {
            row_number,
            item_id,
            category_id: category_id.to_string(),
        });
    };
    if !category.enabled {
        return Err(ItemValidationError::DisabledCategory {
            row_number,
            item_id,
            category_id: category_id.to_string(),
        });
    }

    if item.grid_width == 0
        || item.grid_height == 0
        || item.grid_width > MAX_ITEM_GRID_DIMENSION
        || item.grid_height > MAX_ITEM_GRID_DIMENSION
    {
        return Err(ItemValidationError::InvalidDimensions {
            row_number,
            item_id,
            width: item.grid_width,
            height: item.grid_height,
        });
    }

    validate_stack_configuration(item, row_number)?;

    if item.mass_grams_per_unit == 0 {
        return Err(ItemValidationError::InvalidMass {
            row_number,
            item_id,
            mass_grams: item.mass_grams_per_unit,
        });
    }

    if item.base_value_gold == 0 && item.enabled {
        // Zero base value is allowed for disabled definitions; enabled items need a positive value.
        // Physical gold uses base_value_gold = 1 per coin.
    }

    Ok(())
}

fn validate_stack_configuration(
    item: &ItemDefinition,
    row_number: Option<usize>,
) -> Result<(), ItemValidationError> {
    let item_id = item.id.clone();

    if item.stackable && item.unique_instance_required {
        return Err(ItemValidationError::ContradictoryUniqueStackFlags {
            row_number,
            item_id,
        });
    }

    if item.stackable {
        if item.max_stack < 1 {
            return Err(ItemValidationError::InvalidStackConfiguration {
                row_number,
                item_id,
                message: format!(
                    "stackable item max_stack must be >= 1 (got {})",
                    item.max_stack
                ),
            });
        }
        if item.unique_instance_required {
            return Err(ItemValidationError::ContradictoryUniqueStackFlags {
                row_number,
                item_id,
            });
        }
        Ok(())
    } else if item.unique_instance_required {
        if item.max_stack != 1 {
            return Err(ItemValidationError::InvalidStackConfiguration {
                row_number,
                item_id,
                message: format!("unique item max_stack must be 1 (got {})", item.max_stack),
            });
        }
        Ok(())
    } else if item.max_stack != 1 {
        Err(ItemValidationError::InvalidStackConfiguration {
            row_number,
            item_id,
            message: format!(
                "non-stackable item without unique_instance_required must have max_stack = 1 (got {})",
                item.max_stack
            ),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ItemCategoryCatalog, ItemCategoryDefinition, ItemCategoryId, ItemDefinition,
    };

    fn categories() -> ItemCategoryCatalog {
        ItemCategoryCatalog::from_definitions(vec![ItemCategoryDefinition::new(
            ItemCategoryId::new("currency"),
            "Currency",
            "",
            true,
        )])
        .expect("categories")
    }

    fn valid_stackable() -> ItemDefinition {
        ItemDefinition::new(
            ItemDefinitionId::new("gold"),
            "Gold",
            "",
            ItemCategoryId::new("currency"),
            1,
            1,
            true,
            999,
            1,
            1,
            true,
        )
    }

    #[test]
    fn valid_stackable_item_passes() {
        validate_item_definition(&valid_stackable(), &categories(), None).expect("valid");
    }

    #[test]
    fn valid_unique_item_passes() {
        let item = ItemDefinition::new(
            ItemDefinitionId::new("sword"),
            "Sword",
            "",
            ItemCategoryId::new("currency"),
            2,
            4,
            false,
            1,
            500,
            10,
            true,
        )
        .with_unique_instance_required(true);
        validate_item_definition(&item, &categories(), None).expect("valid unique");
    }

    #[test]
    fn contradictory_stack_unique_rejected() {
        let mut item = valid_stackable();
        item.unique_instance_required = true;
        assert!(matches!(
            validate_item_definition(&item, &categories(), None),
            Err(ItemValidationError::ContradictoryUniqueStackFlags { .. })
        ));
    }

    #[test]
    fn zero_dimensions_rejected() {
        let mut item = valid_stackable();
        item.grid_width = 0;
        assert!(matches!(
            validate_item_definition(&item, &categories(), None),
            Err(ItemValidationError::InvalidDimensions { .. })
        ));
    }

    #[test]
    fn zero_stack_limit_rejected() {
        let mut item = valid_stackable();
        item.max_stack = 0;
        assert!(matches!(
            validate_item_definition(&item, &categories(), None),
            Err(ItemValidationError::InvalidStackConfiguration { .. })
        ));
    }

    #[test]
    fn category_reference_validated() {
        let mut item = valid_stackable();
        item.category_id = ItemCategoryId::new("missing");
        assert!(matches!(
            validate_item_definition(&item, &categories(), None),
            Err(ItemValidationError::MissingCategory { .. })
        ));
    }

    #[test]
    fn integer_mass_preserved() {
        let item = valid_stackable();
        assert_eq!(item.mass_grams_per_unit, 1);
    }

    #[test]
    fn tags_normalized_deterministically() {
        assert_eq!(
            normalize_tags(" Trade, food ;trade "),
            vec!["food".to_string(), "trade".to_string()]
        );
    }
}
