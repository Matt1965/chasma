//! Off-screen death presentation policy (A1).
//!
//! Presentation-only: simulation removes units immediately; corpses are never
//! spawned for units that had no resident render entity at death time.

/// Whether death presentation may begin for a unit that still has a render root.
///
/// Policy (A1):
/// - No new corpse entities are spawned off-screen.
/// - Only existing resident render roots may enter [`super::components::DeathPresentation`].
/// - Simulation removal remains immediate and authoritative.
pub fn may_begin_death_presentation_on_existing_root(render_index_contains_unit: bool) -> bool {
    render_index_contains_unit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_screen_death_does_not_spawn_new_presentation() {
        assert!(!may_begin_death_presentation_on_existing_root(false));
    }

    #[test]
    fn resident_render_root_may_present_death() {
        assert!(may_begin_death_presentation_on_existing_root(true));
    }
}
