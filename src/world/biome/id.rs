use bevy::prelude::*;

/// World-scale biome classification identifier (ADR-024).
///
/// Phase R1 uses color-based classification only. No gameplay behavior is
/// attached to variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Reflect)]
pub enum BiomeId {
    Desert,
    Forest,
    Marsh,
    Plains,
    #[default]
    Unassigned,
}

impl BiomeId {
    pub const fn is_assigned(self) -> bool {
        !matches!(self, Self::Unassigned)
    }

    /// All assigned classification variants (excludes [`Self::Unassigned`]).
    pub const fn all_assigned() -> &'static [Self] {
        &[Self::Desert, Self::Forest, Self::Marsh, Self::Plains]
    }
}
