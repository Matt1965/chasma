//! Formation layout kinds (ADR-035 U10).

/// Spatial layout used to distribute group move targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormationKind {
    /// Units in row/column blocks centered on the click (large groups).
    #[default]
    Grid,
    /// Units arranged on a straight line through the click center.
    Line,
    /// Units evenly spaced on a ring around the click center.
    Circle,
}
