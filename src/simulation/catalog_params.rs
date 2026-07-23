//! Bundled catalog resources for authoritative simulation ticks (ADR-089 I3).
//!
//! Bevy system functions support a limited parameter count. This [`SystemParam`]
//! groups the read-only catalogs passed into [`super::run_simulation_tick`].

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::world::{
    BuildingCatalog, BuildingInteractionProfileCatalog, CorpseSettings, DoodadCatalog,
    FootprintCatalog, InteriorProfileCatalog, InventoryCatalogCtx, InventoryProfileCatalog,
    ItemCatalog, ItemCategoryCatalog, NavigationConfig, UnitCatalog, WeaponCatalog,
    BuildingNavigationBlueprintCatalog,
};

/// Read-only catalogs and settings consumed by [`super::run_simulation_tick`].
#[derive(SystemParam)]
pub struct SimulationCatalogParams<'w> {
    pub unit_catalog: Res<'w, UnitCatalog>,
    pub weapon_catalog: Res<'w, WeaponCatalog>,
    pub doodad_catalog: Res<'w, DoodadCatalog>,
    pub building_catalog: Res<'w, BuildingCatalog>,
    pub footprint_catalog: Res<'w, FootprintCatalog>,
    pub interaction_catalog: Res<'w, BuildingInteractionProfileCatalog>,
    pub nav_config: Res<'w, NavigationConfig>,
    pub interior_catalog: Res<'w, InteriorProfileCatalog>,
    pub nav_blueprint_catalog: Res<'w, BuildingNavigationBlueprintCatalog>,
    pub combat_ai_settings: Res<'w, crate::world::CombatAiSettings>,
    pub item_catalog: Res<'w, ItemCatalog>,
    pub item_categories: Res<'w, ItemCategoryCatalog>,
    pub inventory_profiles: Res<'w, InventoryProfileCatalog>,
    pub corpse_settings: Res<'w, CorpseSettings>,
}

impl SimulationCatalogParams<'_> {
    pub fn inventory_ctx(&self) -> InventoryCatalogCtx<'_> {
        InventoryCatalogCtx::new(
            &self.item_catalog,
            &self.item_categories,
            &self.inventory_profiles,
        )
    }
}
