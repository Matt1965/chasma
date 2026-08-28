use super::facet::RelationshipFacet;
use super::faction::FactionCatalog;
use super::species::SpeciesCatalog;
use crate::world::unit::UnitRecord;

/// Sole authority for assembling a unit's relationship identity facets (ADR-132 Phase 1).
pub fn assemble_relationship_facets(record: &UnitRecord) -> Vec<RelationshipFacet> {
    let mut facets = Vec::with_capacity(3);
    facets.push(RelationshipFacet::Faction(record.faction_id.clone()));
    facets.push(RelationshipFacet::Species(record.species_id.clone()));
    facets.push(RelationshipFacet::Individual(record.id));
    facets
}

/// Resolve faction display text for presentation without leaking machine slugs.
pub fn faction_display_name(
    catalog: &FactionCatalog,
    faction_id: &super::faction::FactionId,
) -> String {
    catalog
        .display_name(faction_id)
        .map(str::to_owned)
        .unwrap_or_else(|| faction_id.as_str().to_string())
}

/// Resolve species display text for presentation without leaking machine slugs.
pub fn species_display_name(
    catalog: &SpeciesCatalog,
    species_id: &super::species::SpeciesId,
) -> String {
    catalog
        .display_name(species_id)
        .map(str::to_owned)
        .unwrap_or_else(|| species_id.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ownership::UnitOwnership;
    use crate::world::unit::UnitRecord;
    use crate::world::{UnitDefinitionId, UnitPlacement, UnitSource, WorldPosition};
    use bevy::prelude::{Quat, Vec3};

    use crate::world::{ChunkCoord, LocalPosition};

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn facets_include_faction_species_and_individual() {
        let record = UnitRecord::new(
            crate::world::UnitId::new(7),
            UnitDefinitionId::new("wolf"),
            UnitPlacement::new(pos(0.0, 0.0), Quat::IDENTITY),
            UnitSource::Authored,
            UnitOwnership::wildlife(),
            5,
            super::super::faction::FactionId::new("wild"),
            super::super::species::SpeciesId::new("wolf"),
        );
        let facets = assemble_relationship_facets(&record);
        assert_eq!(facets.len(), 3);
        assert!(matches!(facets[0], RelationshipFacet::Faction(_)));
        assert!(matches!(facets[1], RelationshipFacet::Species(_)));
        assert!(matches!(facets[2], RelationshipFacet::Individual(id) if id.raw() == 7));
    }
}
