//! Unit ownership and affiliation (ADR-051 O1).

mod controllability;
mod defaults;
mod query;
mod types;

pub use controllability::{
    filter_commandable_unit_ids, filter_selectable_unit_ids, unit_is_commandable,
    unit_is_selectable, SelectionControllabilityPolicy,
};
pub use defaults::{DEFAULT_PLAYER_OWNER_ID, DEFAULT_PLAYER_TEAM_ID};
pub use query::{
    default_ownership_for_source, is_owned_by, is_player_controllable, player_units,
    units_by_affiliation, units_by_owner,
};
pub use types::{Affiliation, OwnerId, TeamId, UnitOwnership};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::UnitDefinition;
    use crate::world::{UnitCatalog, UnitDefinitionId, UnitRenderKey};

    #[test]
    fn definition_faction_tag_is_not_ownership_truth() {
        let mut catalog = UnitCatalog::default();
        let wolf = catalog.get(&UnitDefinitionId::new("wolf")).unwrap().clone();
        assert_eq!(wolf.faction_tag, "Wild");

        let ownership = UnitOwnership::player_default();
        assert_ne!(wolf.faction_tag, ownership.affiliation.label());
    }

    #[test]
    fn faction_metadata_does_not_imply_player_control() {
        let definition = UnitDefinition::new(
            UnitDefinitionId::new("bandit"),
            "Bandit",
            "Bandits",
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1.0,
            "Normal",
            4.0,
            0.5,
            40.0,
            true,
            UnitRenderKey::reserved("bandit"),
        );
        assert!(!definition.faction_tag.is_empty());
        assert_ne!(UnitOwnership::neutral().affiliation, Affiliation::Player);
    }
}
