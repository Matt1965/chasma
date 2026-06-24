//! Runtime unit ownership types (ADR-051 O1).
//!
//! Ownership is simulation truth on [`UnitRecord`]. Catalog [`UnitDefinition::faction_tag`]
//! is design metadata only — never authoritative for control or diplomacy.

use bevy::prelude::*;

/// Direct controller of a unit instance (player, AI script, neutral world logic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
pub struct OwnerId(pub u64);

impl OwnerId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Ally/enemy grouping hook for future combat and diplomacy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
pub struct TeamId(pub u64);

impl TeamId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Broad runtime classification for UI filtering and controllability (O1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum Affiliation {
    /// Local human player — selectable and commandable when owner matches.
    Player,
    /// No hostile intent; not player-commandable by default.
    #[default]
    Neutral,
    /// Future AI opponent — not player-commandable.
    Hostile,
    /// Ambient fauna — not player-commandable.
    Wildlife,
    /// Dev tooling spawn — controllable only when owner is the local player.
    Dev,
    /// Unclassified / legacy restore fallback.
    Unknown,
}

impl Affiliation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Player => "Player",
            Self::Neutral => "Neutral",
            Self::Hostile => "Hostile",
            Self::Wildlife => "Wildlife",
            Self::Dev => "Dev",
            Self::Unknown => "Unknown",
        }
    }
}

/// Authoritative ownership assigned at spawn (O1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct UnitOwnership {
    pub owner_id: Option<OwnerId>,
    pub team_id: Option<TeamId>,
    pub affiliation: Affiliation,
}

impl UnitOwnership {
    pub fn player_default() -> Self {
        Self {
            owner_id: Some(super::defaults::DEFAULT_PLAYER_OWNER_ID),
            team_id: Some(super::defaults::DEFAULT_PLAYER_TEAM_ID),
            affiliation: Affiliation::Player,
        }
    }

    pub fn neutral() -> Self {
        Self {
            owner_id: None,
            team_id: None,
            affiliation: Affiliation::Neutral,
        }
    }

    pub fn hostile() -> Self {
        Self {
            owner_id: None,
            team_id: None,
            affiliation: Affiliation::Hostile,
        }
    }

    pub fn wildlife() -> Self {
        Self {
            owner_id: None,
            team_id: None,
            affiliation: Affiliation::Wildlife,
        }
    }

    pub fn dev_local_player() -> Self {
        Self {
            owner_id: Some(super::defaults::DEFAULT_PLAYER_OWNER_ID),
            team_id: Some(super::defaults::DEFAULT_PLAYER_TEAM_ID),
            affiliation: Affiliation::Dev,
        }
    }

    pub fn with_affiliation(affiliation: Affiliation) -> Self {
        match affiliation {
            Affiliation::Player => Self::player_default(),
            Affiliation::Dev => Self::dev_local_player(),
            Affiliation::Hostile => Self::hostile(),
            Affiliation::Wildlife => Self::wildlife(),
            Affiliation::Neutral | Affiliation::Unknown => Self::neutral(),
        }
    }
}

impl Default for UnitOwnership {
    fn default() -> Self {
        Self::neutral()
    }
}
