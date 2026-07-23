//! Indexed, cached catalog browsing for dev mode (ADR-047).

use std::collections::HashSet;

use bevy::prelude::*;

use crate::world::{BuildingCatalog, DoodadCatalog, DoodadDefinition, UnitCatalog, UnitDefinition};

use super::catalog_browser::{CatalogBrowserEntry, filter_catalog_entries};
use super::dev_mode::{DefinitionId, DevTab, SpawnMode};

/// Pre-built rows for one catalog type.
#[derive(Debug, Clone, Default)]
pub struct CatalogIndexSlice {
    pub all: Vec<CatalogBrowserEntry>,
    pub catalog_len: usize,
}

/// In-memory index rebuilt when catalog contents change.
#[derive(Resource, Debug, Default)]
pub struct CatalogBrowseIndex {
    pub units: CatalogIndexSlice,
    pub doodads: CatalogIndexSlice,
}

impl CatalogBrowseIndex {
    pub fn sync(&mut self, unit_catalog: &UnitCatalog, doodad_catalog: &DoodadCatalog) {
        self.units.all = unit_catalog.definitions().iter().map(unit_row).collect();
        self.units.catalog_len = unit_catalog.len();
        self.doodads.all = doodad_catalog
            .definitions()
            .iter()
            .map(doodad_row)
            .collect();
        self.doodads.catalog_len = doodad_catalog.len();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    tab: DevTab,
    enabled_only: bool,
    query: String,
    favorites_hash: u64,
    building_revision: u64,
}

/// Cached filter output keyed by tab + query + favorites.
#[derive(Resource, Debug, Default)]
pub struct CatalogFilterCache {
    key: Option<CacheKey>,
    entries: Vec<CatalogBrowserEntry>,
}

const SEARCH_DEBOUNCE_FRAMES: u32 = 4;

/// Session search debounce — raw input vs filtered query.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub struct DevSearchDebounce {
    pub raw_query: String,
    pub filtered_query: String,
    pub frames_until_settle: u32,
}

impl DevSearchDebounce {
    pub fn note_input(&mut self, raw: &str) {
        self.raw_query = raw.to_string();
        self.frames_until_settle = SEARCH_DEBOUNCE_FRAMES;
    }

    pub fn tick(&mut self) -> bool {
        if self.frames_until_settle == 0 {
            return false;
        }
        self.frames_until_settle -= 1;
        if self.frames_until_settle == 0 && self.filtered_query != self.raw_query {
            self.filtered_query = self.raw_query.clone();
            return true;
        }
        false
    }

    pub fn force_sync(&mut self, raw: &str) {
        self.raw_query = raw.to_string();
        self.filtered_query = raw.to_string();
        self.frames_until_settle = 0;
    }
}

fn favorites_hash(favorites: &HashSet<DefinitionId>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut ids: Vec<_> = favorites.iter().map(DefinitionId::id_str).collect();
    ids.sort_unstable();
    ids.hash(&mut hasher);
    hasher.finish()
}

/// Filtered catalog rows with favorites pinned first; results cached per query.
pub fn browse_catalog_entries<'a>(
    index: &'a CatalogBrowseIndex,
    cache: &'a mut CatalogFilterCache,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    building_revision: u64,
    tab: DevTab,
    spawn_mode: SpawnMode,
    search_query: &str,
    enabled_only: bool,
    favorites: &HashSet<DefinitionId>,
) -> &'a [CatalogBrowserEntry] {
    let key = CacheKey {
        tab,
        enabled_only,
        query: search_query.to_string(),
        favorites_hash: favorites_hash(favorites),
        building_revision,
    };
    if cache.key.as_ref() == Some(&key) {
        return &cache.entries;
    }

    let entries = if matches!(tab, DevTab::Units | DevTab::Doodads) {
        let base = match tab {
            DevTab::Units => filtered_from_index(&index.units, enabled_only, search_query),
            DevTab::Doodads => filtered_from_index(&index.doodads, enabled_only, search_query),
            _ => Vec::new(),
        };
        pin_favorites(base, favorites)
    } else {
        filter_catalog_entries(
            unit_catalog,
            doodad_catalog,
            building_catalog,
            tab,
            spawn_mode,
            search_query,
            enabled_only,
        )
    };

    cache.key = Some(key);
    cache.entries = entries;
    &cache.entries
}

fn filtered_from_index(
    slice: &CatalogIndexSlice,
    enabled_only: bool,
    search_query: &str,
) -> Vec<CatalogBrowserEntry> {
    let query = search_query.trim().to_ascii_lowercase();
    slice
        .all
        .iter()
        .filter(|entry| !enabled_only || entry.enabled)
        .filter(|entry| {
            if query.is_empty() {
                return true;
            }
            entry.label.to_ascii_lowercase().contains(&query)
                || entry.render_key.to_ascii_lowercase().contains(&query)
                || entry.category.to_ascii_lowercase().contains(&query)
                || entry
                    .definition
                    .id_str()
                    .to_ascii_lowercase()
                    .contains(&query)
        })
        .cloned()
        .collect()
}

fn pin_favorites(
    entries: Vec<CatalogBrowserEntry>,
    favorites: &HashSet<DefinitionId>,
) -> Vec<CatalogBrowserEntry> {
    if favorites.is_empty() {
        let mut sorted = entries;
        sorted.sort_by(|a, b| a.label.cmp(&b.label));
        return sorted;
    }
    let (mut pinned, mut rest): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .partition(|entry| favorites.contains(&entry.definition));
    pinned.sort_by(|a, b| a.label.cmp(&b.label));
    rest.sort_by(|a, b| a.label.cmp(&b.label));
    pinned.extend(rest);
    pinned
}

fn unit_row(def: &UnitDefinition) -> CatalogBrowserEntry {
    CatalogBrowserEntry {
        definition: DefinitionId::Unit(def.id.clone()),
        label: def.display_name.clone(),
        category: def.faction_tag.clone(),
        render_key: def.render_key.0.clone().unwrap_or_default(),
        enabled: def.enabled,
    }
}

fn doodad_row(def: &DoodadDefinition) -> CatalogBrowserEntry {
    CatalogBrowserEntry {
        definition: DefinitionId::Doodad(def.id.clone()),
        label: def.display_name.clone(),
        category: format!("{:?}", def.kind),
        render_key: def.render_key.0.clone().unwrap_or_default(),
        enabled: def.enabled,
    }
}

/// Keep browse index in sync when catalog lengths change.
pub fn sync_catalog_browse_index(
    unit_catalog: Res<UnitCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    mut index: ResMut<CatalogBrowseIndex>,
) {
    if unit_catalog.is_changed() || doodad_catalog.is_changed() {
        index.sync(&unit_catalog, &doodad_catalog);
    }
}

/// Advance search debounce from raw [`DevModeState::search_query`].
pub fn tick_dev_search_debounce(
    dev_state: Res<super::dev_mode::DevModeState>,
    mut debounce: ResMut<DevSearchDebounce>,
) {
    if !dev_state.enabled {
        return;
    }
    if debounce.raw_query != dev_state.search_query {
        debounce.note_input(&dev_state.search_query);
    }
    debounce.tick();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BuildingCatalog, UnitCatalog, starter_unit_definitions};

    #[test]
    fn search_debounce_delays_filtered_query() {
        let mut debounce = DevSearchDebounce::default();
        debounce.note_input("wolf");
        assert_eq!(debounce.filtered_query, "");
        for _ in 0..SEARCH_DEBOUNCE_FRAMES - 1 {
            assert!(!debounce.tick());
        }
        assert!(debounce.tick());
        assert_eq!(debounce.filtered_query, "wolf");
    }

    #[test]
    fn catalog_filter_cache_avoids_recompute() {
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let mut index = CatalogBrowseIndex::default();
        index.sync(&catalog, &DoodadCatalog::default());
        let mut cache = CatalogFilterCache::default();
        let favorites = HashSet::new();
        let first_ptr = browse_catalog_entries(
            &index,
            &mut cache,
            &catalog,
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            0,
            DevTab::Units,
            SpawnMode::Unit,
            "wolf",
            true,
            &favorites,
        )
        .as_ptr() as usize;
        let second_ptr = browse_catalog_entries(
            &index,
            &mut cache,
            &catalog,
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            0,
            DevTab::Units,
            SpawnMode::Unit,
            "wolf",
            true,
            &favorites,
        )
        .as_ptr() as usize;
        assert_eq!(first_ptr, second_ptr);
    }

    #[test]
    fn favorites_pinned_at_top() {
        let catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let mut index = CatalogBrowseIndex::default();
        index.sync(&catalog, &DoodadCatalog::default());
        let mut cache = CatalogFilterCache::default();
        let mut favorites = HashSet::new();
        favorites.insert(DefinitionId::Unit(crate::world::UnitDefinitionId::new(
            "deer",
        )));
        let entries = browse_catalog_entries(
            &index,
            &mut cache,
            &catalog,
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            0,
            DevTab::Units,
            SpawnMode::Unit,
            "",
            true,
            &favorites,
        );
        assert!(matches!(
            entries.first().map(|e| e.definition.id_str()),
            Some("deer")
        ));
    }
}
