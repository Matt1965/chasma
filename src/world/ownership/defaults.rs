//! Default player owner/team ids (ADR-051 O1).

use super::types::{OwnerId, TeamId};

/// Stable local human player owner id (single-player foundation).
pub const DEFAULT_PLAYER_OWNER_ID: OwnerId = OwnerId::new(1);

/// Stable local human player team id.
pub const DEFAULT_PLAYER_TEAM_ID: TeamId = TeamId::new(1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_ids_are_non_zero() {
        assert_ne!(DEFAULT_PLAYER_OWNER_ID.raw(), 0);
        assert_ne!(DEFAULT_PLAYER_TEAM_ID.raw(), 0);
    }
}
