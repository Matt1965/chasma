//! Spawn archetype presets — editor-authored templates on base unit/building definitions.
//!
//! Archetypes are not alternate definitions. They supply authored spawn-time differences
//! resolved through [`resolve_unit_spawn_spec`] / [`resolve_building_spawn_spec`].

mod apply;
mod building;
mod capture;
mod persistence;
mod resolve;
mod unit;

#[cfg(test)]
mod tests;

pub use apply::{apply_unit_archetype_spawn_overrides, ArchetypeApplyError};
pub use building::{
    BuildingArchetypeCatalog, BuildingArchetypeCatalogError, BuildingArchetypeDefinition,
    BuildingArchetypeId, BuildingArchetypeSnapshot, unique_building_archetype_id,
};
pub use capture::{
    ArchetypeCaptureError, CapturedUnitArchetypeTemplate, build_building_archetype_definition,
    build_unit_archetype_definition, capture_building_archetype_snapshot,
    capture_unit_archetype_template,
};
pub use persistence::{
    ArchetypePersistenceError, BUILDING_ARCHETYPES_RON_PATH, UNIT_ARCHETYPES_RON_PATH,
    load_building_archetype_catalog_from_ron, load_dev_building_archetype_catalog,
    load_dev_unit_archetype_catalog, load_unit_archetype_catalog_from_ron,
    save_building_archetype_catalog_to_ron, save_unit_archetype_catalog_to_ron,
};
pub use resolve::{
    ArchetypeResolveError, ResolvedBuildingSpawnSpec, ResolvedUnitSpawnSpec,
    resolve_building_spawn_spec, resolve_unit_spawn_spec,
};
pub use unit::{
    ArchetypeEquipmentEntry, ArchetypeInventoryStack, UnitArchetypeCatalog,
    UnitArchetypeCatalogError, UnitArchetypeDefinition, UnitArchetypeId, slugify_archetype_id,
    unique_unit_archetype_id, validate_gold_range,
};
