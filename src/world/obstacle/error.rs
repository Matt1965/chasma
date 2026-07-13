//! Obstacle query errors (REVIEW-B6, ADR-031).

use core::fmt;

use crate::world::DoodadDefinitionId;

/// Structured obstacle query failure (fail-closed policy).
#[derive(Debug, Clone, PartialEq)]
pub enum ObstacleQueryError {
    /// A doodad record references a definition missing from the catalog.
    MissingDoodadDefinition { definition_id: DoodadDefinitionId },
    /// Blocking radius resolved from fallback data was invalid.
    InvalidBlockingRadius { radius_meters: f32 },
    /// Record data was insufficient to evaluate blocking safely.
    CorruptDoodadRecord,
    /// Generalized occupancy evaluation failed.
    Occupancy(crate::world::OccupancyError),
}

impl fmt::Display for ObstacleQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDoodadDefinition { definition_id } => write!(
                f,
                "doodad definition `{}` missing from catalog",
                definition_id.as_str()
            ),
            Self::InvalidBlockingRadius { radius_meters } => {
                write!(f, "invalid blocking radius {radius_meters} m")
            }
            Self::CorruptDoodadRecord => write!(f, "corrupt doodad record for obstacle query"),
            Self::Occupancy(error) => write!(f, "occupancy error: {error:?}"),
        }
    }
}

impl std::error::Error for ObstacleQueryError {}
