//! Shared helpers for occupancy/passability tests.

use std::sync::OnceLock;

use crate::world::{BuildingCatalog, DoodadCatalog, FootprintCatalog, PassabilityCatalogs};

struct StaticPassabilityStores {
    doodad: DoodadCatalog,
    building: BuildingCatalog,
    footprint: FootprintCatalog,
}

static STATIC_PASSABILITY: OnceLock<StaticPassabilityStores> = OnceLock::new();

fn static_stores() -> &'static StaticPassabilityStores {
    STATIC_PASSABILITY.get_or_init(|| StaticPassabilityStores {
        doodad: DoodadCatalog::default(),
        building: BuildingCatalog::default(),
        footprint: FootprintCatalog::default(),
    })
}

/// Default passability catalogs backed by process-lifetime storage (test-only).
pub fn default_passability() -> PassabilityCatalogs<'static> {
    let stores = static_stores();
    PassabilityCatalogs {
        doodad: &stores.doodad,
        building: &stores.building,
        footprint: &stores.footprint,
    }
}

pub fn default_building_catalog() -> &'static BuildingCatalog {
    &static_stores().building
}

pub fn default_footprint_catalog() -> &'static FootprintCatalog {
    &static_stores().footprint
}

/// Holds owned catalogs for tests that need custom doodad definitions.
pub struct TestPassabilityBundle {
    pub doodad: DoodadCatalog,
    pub building: BuildingCatalog,
    pub footprint: FootprintCatalog,
}

impl Default for TestPassabilityBundle {
    fn default() -> Self {
        Self::new()
    }
}

impl TestPassabilityBundle {
    pub fn new() -> Self {
        Self {
            doodad: DoodadCatalog::default(),
            building: BuildingCatalog::default(),
            footprint: FootprintCatalog::default(),
        }
    }

    pub fn with_doodad(doodad: DoodadCatalog) -> Self {
        Self {
            doodad,
            building: BuildingCatalog::default(),
            footprint: FootprintCatalog::default(),
        }
    }

    pub fn catalogs(&self) -> PassabilityCatalogs<'_> {
        PassabilityCatalogs {
            doodad: &self.doodad,
            building: &self.building,
            footprint: &self.footprint,
        }
    }

    pub fn catalogs_for<'a>(&'a self, doodad: &'a DoodadCatalog) -> PassabilityCatalogs<'a> {
        PassabilityCatalogs {
            doodad,
            building: &self.building,
            footprint: &self.footprint,
        }
    }
}
