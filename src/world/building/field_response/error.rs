use super::id::FieldResponseProfileId;

/// Catalog and profile validation errors (ADR-104 TF4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldResponseProfileError {
    DuplicateId(FieldResponseProfileId),
    ProfileMissing(FieldResponseProfileId),
    ProfileDisabled(FieldResponseProfileId),
    PointsEmpty(FieldResponseProfileId),
    PointsUnsorted(FieldResponseProfileId),
    DuplicatePoint {
        profile_id: FieldResponseProfileId,
        field_value: u16,
    },
    TooFewPoints(FieldResponseProfileId),
    EfficiencyOutOfRange {
        profile_id: FieldResponseProfileId,
        efficiency_basis_points: u32,
    },
    InvalidProfileId(String),
    RonIo(String),
    RonParse(String),
}

impl std::fmt::Display for FieldResponseProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate response profile id `{id}`"),
            Self::ProfileMissing(id) => write!(f, "response profile `{id}` missing"),
            Self::ProfileDisabled(id) => write!(f, "response profile `{id}` disabled"),
            Self::PointsEmpty(id) => write!(f, "response profile `{id}` has no points"),
            Self::PointsUnsorted(id) => write!(f, "response profile `{id}` points not sorted"),
            Self::DuplicatePoint {
                profile_id,
                field_value,
            } => write!(
                f,
                "duplicate point at field value {field_value} in profile `{profile_id}`"
            ),
            Self::TooFewPoints(id) => {
                write!(f, "response profile `{id}` needs at least two points")
            }
            Self::EfficiencyOutOfRange {
                profile_id,
                efficiency_basis_points,
            } => write!(
                f,
                "efficiency {efficiency_basis_points} bp out of range for profile `{profile_id}`"
            ),
            Self::InvalidProfileId(id) => write!(f, "invalid response profile id `{id}`"),
            Self::RonIo(msg) => write!(f, "response profile catalog io error: {msg}"),
            Self::RonParse(msg) => write!(f, "response profile catalog parse error: {msg}"),
        }
    }
}

/// Runtime evaluation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldResponseEvaluationError {
    ProfileMissing(FieldResponseProfileId),
    ProfileDisabled(FieldResponseProfileId),
    PointsEmpty(FieldResponseProfileId),
    MalformedProfile(FieldResponseProfileId),
}

impl std::fmt::Display for FieldResponseEvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProfileMissing(id) => write!(f, "response profile `{id}` missing"),
            Self::ProfileDisabled(id) => write!(f, "response profile `{id}` disabled"),
            Self::PointsEmpty(id) => write!(f, "response profile `{id}` has no points"),
            Self::MalformedProfile(id) => write!(f, "response profile `{id}` malformed"),
        }
    }
}
