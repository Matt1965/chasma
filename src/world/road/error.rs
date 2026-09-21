use std::fmt;

use super::id::RoadId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoadError {
    UnsupportedSchemaVersion { found: u32, expected: u32 },
    DuplicateRoadId(RoadId),
    DuplicateJunctionId(super::id::JunctionId),
    InvalidRoadId(String),
    InvalidJunctionId(String),
    InvalidControlPoint(String),
    InvalidControlPointCount { road_id: RoadId, count: usize },
    UnknownStyle(super::style::RoadStyleId),
    InvalidStyleValue(String),
    InvalidJunctionReference(String),
    DuplicateJunctionMember {
        junction_id: super::id::JunctionId,
        road_id: RoadId,
    },
    AttachmentMismatch(String),
}

impl fmt::Display for RoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { found, expected } => {
                write!(
                    f,
                    "unsupported road network schema version {found} (expected {expected})"
                )
            }
            Self::DuplicateRoadId(id) => write!(f, "duplicate road id {id}"),
            Self::DuplicateJunctionId(id) => write!(f, "duplicate junction id {id}"),
            Self::InvalidRoadId(message) => write!(f, "invalid road id: {message}"),
            Self::InvalidJunctionId(message) => write!(f, "invalid junction id: {message}"),
            Self::InvalidControlPoint(message) => write!(f, "invalid control point: {message}"),
            Self::InvalidControlPointCount { road_id, count } => {
                write!(
                    f,
                    "road {road_id} has invalid control point count {count} (minimum 2)"
                )
            }
            Self::UnknownStyle(style) => write!(f, "unknown road style {style:?}"),
            Self::InvalidStyleValue(message) => write!(f, "invalid road style value: {message}"),
            Self::InvalidJunctionReference(message) => {
                write!(f, "invalid junction reference: {message}")
            }
            Self::DuplicateJunctionMember { junction_id, road_id } => write!(
                f,
                "junction {junction_id} references road {road_id} more than once"
            ),
            Self::AttachmentMismatch(message) => write!(f, "road attachment mismatch: {message}"),
        }
    }
}

impl std::error::Error for RoadError {}

#[derive(Debug)]
pub enum RoadLoadError {
    Io(std::io::Error),
    Parse(String),
    Validation(RoadError),
}

impl fmt::Display for RoadLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read road network: {error}"),
            Self::Parse(message) => write!(f, "failed to parse road network: {message}"),
            Self::Validation(error) => write!(f, "invalid road network: {error}"),
        }
    }
}

impl std::error::Error for RoadLoadError {}

impl From<std::io::Error> for RoadLoadError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<RoadError> for RoadLoadError {
    fn from(value: RoadError) -> Self {
        Self::Validation(value)
    }
}
