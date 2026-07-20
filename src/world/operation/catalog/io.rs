//! Future input/output catalog seams (EP3). Not executed at runtime.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::ItemDefinitionId;
use crate::world::TerrainFieldId;
use crate::world::building::inventory_binding::BuildingInventoryBindingId;

/// Future recipe input reference (EP5).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct OperationInputDefinition {
    pub item_id: ItemDefinitionId,
    pub quantity: u32,
    /// Authored building inventory channel to withdraw from (EP4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_binding: Option<BuildingInventoryBindingId>,
}

/// Non-item output kinds for future phases (EP3 seam).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum OperationEffectKind {
    Research {
        topic_id: String,
    },
    Training {
        skill_id: String,
    },
    Medical {
        treatment_id: String,
    },
    SettlementInfluence {
        amount: u32,
    },
    BuildingMaintenance {
        amount: u32,
    },
}

/// Future operation output — item or non-item effect (EP3 seam).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum OperationOutputDefinition {
    Item {
        item_id: ItemDefinitionId,
        quantity: u32,
        /// Authored building inventory channel to deposit into (EP4).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        destination_binding: Option<BuildingInventoryBindingId>,
    },
    Effect(OperationEffectKind),
}

/// Terrain field requirement for extraction/processing efficiency (EP6).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct OperationTerrainRequirementRef {
    pub field_id: TerrainFieldId,
    pub minimum_average_percent: u8,
}

/// Future tool requirement reference (EP5).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct OperationToolRequirementRef {
    pub tool_category_id: String,
}

/// Future power requirement reference (EP5).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct OperationPowerRequirementRef {
    pub watts: u32,
}

/// Future skill requirement reference (EP5).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct OperationSkillRequirementRef {
    pub skill_id: String,
    pub minimum_level: u32,
}

impl OperationTerrainRequirementRef {
    pub fn validate(&self) -> Result<(), OperationIoValidationError> {
        if self.minimum_average_percent > 100 {
            return Err(OperationIoValidationError::InvalidTerrainMinimumPercent {
                field_id: self.field_id.clone(),
                minimum_average_percent: self.minimum_average_percent,
            });
        }
        Ok(())
    }
}

impl OperationInputDefinition {
    pub fn validate(&self) -> Result<(), OperationIoValidationError> {
        if self.quantity == 0 {
            return Err(OperationIoValidationError::ZeroInputQuantity {
                item_id: self.item_id.clone(),
            });
        }
        Ok(())
    }
}

impl OperationOutputDefinition {
    pub fn validate(&self) -> Result<(), OperationIoValidationError> {
        match self {
            Self::Item {
                quantity,
                item_id,
                destination_binding: _,
            } => {
                if *quantity == 0 {
                    return Err(OperationIoValidationError::ZeroOutputQuantity {
                        item_id: item_id.clone(),
                    });
                }
            }
            Self::Effect(OperationEffectKind::Research { topic_id }) => {
                if topic_id.is_empty() {
                    return Err(OperationIoValidationError::EmptyEffectIdentifier);
                }
            }
            Self::Effect(OperationEffectKind::Training { skill_id }) => {
                if skill_id.is_empty() {
                    return Err(OperationIoValidationError::EmptyEffectIdentifier);
                }
            }
            Self::Effect(OperationEffectKind::Medical { treatment_id }) => {
                if treatment_id.is_empty() {
                    return Err(OperationIoValidationError::EmptyEffectIdentifier);
                }
            }
            Self::Effect(OperationEffectKind::SettlementInfluence { amount })
            | Self::Effect(OperationEffectKind::BuildingMaintenance { amount }) => {
                if *amount == 0 {
                    return Err(OperationIoValidationError::ZeroEffectAmount);
                }
            }
        }
        Ok(())
    }
}

/// IO validation failures for catalog authoring (EP3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationIoValidationError {
    ZeroInputQuantity { item_id: ItemDefinitionId },
    ZeroOutputQuantity { item_id: ItemDefinitionId },
    ZeroEffectAmount,
    EmptyEffectIdentifier,
    InvalidTerrainMinimumPercent {
        field_id: TerrainFieldId,
        minimum_average_percent: u8,
    },
    DuplicateTerrainField {
        field_id: TerrainFieldId,
    },
}
