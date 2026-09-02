//! Settlement creation errors (ADR-133).

use std::fmt;

use super::super::error::TreasuryError;
use super::super::id::SettlementId;
use super::id::SettlementAnchorId;
use crate::world::BuildingId;

#[derive(Debug, Clone, PartialEq)]
pub enum SettlementCreationError {
    OverlapsExisting {
        existing_settlement_id: SettlementId,
        distance_meters: f32,
        required_separation_meters: f32,
    },
    DuplicateAnchorId(SettlementAnchorId),
    DuplicateSettlementId(SettlementId),
    Treasury(TreasuryError),
    BuildingNotFound(BuildingId),
    BuildingNotSettlementCapable(BuildingId),
    SettlementAlreadyLinked(BuildingId),
}

impl fmt::Display for SettlementCreationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OverlapsExisting {
                existing_settlement_id,
                distance_meters,
                required_separation_meters,
            } => write!(
                f,
                "settlement placement overlaps settlement {} (distance {:.2}m < required {:.2}m)",
                existing_settlement_id.raw(),
                distance_meters,
                required_separation_meters
            ),
            Self::DuplicateAnchorId(id) => write!(f, "duplicate settlement anchor id {id:?}"),
            Self::DuplicateSettlementId(id) => write!(f, "duplicate settlement id {id:?}"),
            Self::Treasury(error) => write!(f, "{error}"),
            Self::BuildingNotFound(id) => write!(f, "building {id:?} not found"),
            Self::BuildingNotSettlementCapable(id) => {
                write!(f, "building {id:?} cannot host a settlement treasury")
            }
            Self::SettlementAlreadyLinked(id) => {
                write!(f, "building {id:?} already linked to a settlement")
            }
        }
    }
}

impl std::error::Error for SettlementCreationError {}

impl From<TreasuryError> for SettlementCreationError {
    fn from(value: TreasuryError) -> Self {
        Self::Treasury(value)
    }
}
