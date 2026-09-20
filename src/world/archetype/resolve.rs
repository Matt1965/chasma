use crate::world::building::BuildingLifecycleState;

use crate::world::ownership::UnitOwnership;
use crate::world::BuildingOwnership;

use crate::world::{Affiliation, BuildingDefinitionId, UnitCatalog, UnitDefinitionId};



use super::building::{

    BuildingArchetypeCatalog, BuildingArchetypeDefinition, BuildingArchetypeId,

    BuildingArchetypeSnapshot,

};

use super::unit::{

    ArchetypeEquipmentEntry, ArchetypeInventoryStack, UnitArchetypeCatalog,

    UnitArchetypeDefinition, UnitArchetypeId,

};



/// Effective unit spawn request after base definition + optional archetype resolution.

#[derive(Debug, Clone, PartialEq)]

pub struct ResolvedUnitSpawnSpec {

    pub definition_id: UnitDefinitionId,

    pub ownership: UnitOwnership,

    pub equipment: Vec<ArchetypeEquipmentEntry>,

    pub inventory_stacks: Vec<ArchetypeInventoryStack>,

    pub gold_min: u32,

    pub gold_max: u32,

}



/// Effective building spawn request after base definition + optional archetype resolution.

#[derive(Debug, Clone, PartialEq)]

pub struct ResolvedBuildingSpawnSpec {

    pub definition_id: BuildingDefinitionId,

    pub ownership: BuildingOwnership,

    pub lifecycle_state: BuildingLifecycleState,

    pub snapshot: Option<BuildingArchetypeSnapshot>,

    pub archetype: Option<BuildingArchetypeDefinition>,

}



#[derive(Debug, Clone, PartialEq, Eq)]

pub enum ArchetypeResolveError {

    BaseUnitNotFound(UnitDefinitionId),

    BaseBuildingNotFound(BuildingDefinitionId),

    ArchetypeNotFound {

        kind: &'static str,

        id: String,

    },

    ArchetypeDisabled {

        kind: &'static str,

        id: String,

    },

    ArchetypeNotApplicable {

        kind: &'static str,

        id: String,

        base_key: String,

    },

}



pub fn resolve_unit_spawn_spec(

    base_definition_id: &UnitDefinitionId,

    archetype_id: Option<&UnitArchetypeId>,

    dev_affiliation: Affiliation,

    unit_catalog: &UnitCatalog,

    archetype_catalog: &UnitArchetypeCatalog,

) -> Result<ResolvedUnitSpawnSpec, ArchetypeResolveError> {

    if unit_catalog.get(base_definition_id).is_none() {

        return Err(ArchetypeResolveError::BaseUnitNotFound(

            base_definition_id.clone(),

        ));

    }



    let archetype = match archetype_id {

        None => {

            return Ok(ResolvedUnitSpawnSpec {

                definition_id: base_definition_id.clone(),

                ownership: UnitOwnership::with_affiliation(dev_affiliation),

                equipment: Vec::new(),

                inventory_stacks: Vec::new(),

                gold_min: 0,

                gold_max: 0,

            });

        }

        Some(id) => archetype_catalog

            .get(id)

            .ok_or_else(|| ArchetypeResolveError::ArchetypeNotFound {

                kind: "unit",

                id: id.as_str().to_string(),

            })?,

    };



    validate_unit_archetype(base_definition_id, archetype, unit_catalog)?;



    let affiliation = archetype

        .affiliation_override

        .unwrap_or(dev_affiliation);



    Ok(ResolvedUnitSpawnSpec {

        definition_id: base_definition_id.clone(),

        ownership: UnitOwnership::with_affiliation(affiliation),

        equipment: archetype.equipment.clone(),

        inventory_stacks: archetype.inventory_stacks.clone(),

        gold_min: archetype.gold_min,

        gold_max: archetype.gold_max,

    })

}



pub fn resolve_building_spawn_spec(

    base_definition_id: &BuildingDefinitionId,

    archetype_id: Option<&BuildingArchetypeId>,

    dev_affiliation: Affiliation,

    building_catalog: &crate::world::BuildingCatalog,

    archetype_catalog: &BuildingArchetypeCatalog,

) -> Result<ResolvedBuildingSpawnSpec, ArchetypeResolveError> {

    if building_catalog.get(base_definition_id).is_none() {

        return Err(ArchetypeResolveError::BaseBuildingNotFound(

            base_definition_id.clone(),

        ));

    }



    let archetype = match archetype_id {

        None => {

            return Ok(ResolvedBuildingSpawnSpec {

                definition_id: base_definition_id.clone(),

                ownership: BuildingOwnership::with_affiliation(dev_affiliation),

                lifecycle_state: BuildingLifecycleState::Complete,

                snapshot: None,

                archetype: None,

            });

        }

        Some(id) => archetype_catalog

            .get(id)

            .ok_or_else(|| ArchetypeResolveError::ArchetypeNotFound {

                kind: "building",

                id: id.as_str().to_string(),

            })?,

    };



    validate_building_archetype(base_definition_id, archetype)?;



    let snapshot = &archetype.snapshot;

    let affiliation = snapshot.affiliation;



    Ok(ResolvedBuildingSpawnSpec {

        definition_id: base_definition_id.clone(),

        ownership: BuildingOwnership {

            owner_id: snapshot.owner_id,

            team_id: snapshot.team_id,

            affiliation,

        },

        lifecycle_state: snapshot.lifecycle_state,

        snapshot: Some(snapshot.clone()),

        archetype: Some(archetype.clone()),

    })

}



fn validate_unit_archetype(

    base_definition_id: &UnitDefinitionId,

    archetype: &UnitArchetypeDefinition,

    unit_catalog: &UnitCatalog,

) -> Result<(), ArchetypeResolveError> {

    if !archetype.enabled {

        return Err(ArchetypeResolveError::ArchetypeDisabled {

            kind: "unit",

            id: archetype.id.as_str().to_string(),

        });

    }

    if !archetype.applies_to_unit(base_definition_id, unit_catalog) {

        return Err(ArchetypeResolveError::ArchetypeNotApplicable {

            kind: "unit",

            id: archetype.id.as_str().to_string(),

            base_key: base_definition_id.as_str().to_string(),

        });

    }

    Ok(())

}



fn validate_building_archetype(

    base_definition_id: &BuildingDefinitionId,

    archetype: &BuildingArchetypeDefinition,

) -> Result<(), ArchetypeResolveError> {

    if !archetype.enabled {

        return Err(ArchetypeResolveError::ArchetypeDisabled {

            kind: "building",

            id: archetype.id.as_str().to_string(),

        });

    }

    if !archetype.applies_to(base_definition_id) {

        return Err(ArchetypeResolveError::ArchetypeNotApplicable {

            kind: "building",

            id: archetype.id.as_str().to_string(),

            base_key: base_definition_id.as_str().to_string(),

        });

    }

    Ok(())

}


