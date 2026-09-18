//! Preview respawn helpers shared with CG3.

use crate::world::UnitAppearance;

pub fn appearance_requires_preview_respawn(current: &UnitAppearance, next: &UnitAppearance) -> bool {
    current.profile_id != next.profile_id || current.body_variant_id != next.body_variant_id
}
