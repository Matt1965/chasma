//! In-memory catalog filtering for dev browser (ADR-043).

use crate::world::{
    DoodadCatalog, DoodadDefinition, DoodadKind, UnitCatalog, UnitDefinition,
};

use super::dev_mode::{DefinitionId, DevTab, SpawnMode};

/// One row in the dev catalog list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogBrowserEntry {
    pub definition: DefinitionId,
    pub label: String,
    pub category: String,
    pub render_key: String,
    pub enabled: bool,
}

/// Filter catalog definitions for the active dev tab.
pub fn filter_catalog_entries(
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    tab: DevTab,
    spawn_mode: SpawnMode,
    search_query: &str,
    enabled_only: bool,
) -> Vec<CatalogBrowserEntry> {
    let query = search_query.trim().to_ascii_lowercase();
    let mut entries = match tab {
        DevTab::Units => unit_entries(unit_catalog, enabled_only),
        DevTab::Doodads => doodad_entries(doodad_catalog, enabled_only),
        DevTab::Debug | DevTab::Placement | DevTab::Scenes | DevTab::Inspector | DevTab::WorldTools => {
            Vec::new()
        }
    };

    if matches!(tab, DevTab::Units | DevTab::Doodads) {
        // Keep spawn mode aligned when browsing a single-type tab.
        let _ = spawn_mode;
    }

    if !query.is_empty() {
        entries.retain(|entry| {
            entry.label.to_ascii_lowercase().contains(&query)
                || entry.render_key.to_ascii_lowercase().contains(&query)
                || entry.category.to_ascii_lowercase().contains(&query)
                || entry
                    .definition
                    .id_str()
                    .to_ascii_lowercase()
                    .contains(&query)
        });
    }

    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries
}

fn unit_entries(catalog: &UnitCatalog, enabled_only: bool) -> Vec<CatalogBrowserEntry> {
    catalog
        .definitions()
        .iter()
        .filter(|def| !enabled_only || def.enabled)
        .map(unit_row)
        .collect()
}

fn doodad_entries(catalog: &DoodadCatalog, enabled_only: bool) -> Vec<CatalogBrowserEntry> {
    catalog
        .definitions()
        .iter()
        .filter(|def| !enabled_only || def.enabled)
        .map(doodad_row)
        .collect()
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
        category: doodad_kind_label(def.kind),
        render_key: def.render_key.0.clone().unwrap_or_default(),
        enabled: def.enabled,
    }
}

fn doodad_kind_label(kind: DoodadKind) -> String {
    format!("{kind:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{DoodadCatalog, UnitDefinition, UnitDefinitionId};

    #[test]
    fn catalog_filter_returns_unit_subset_by_search() {
        let catalog = UnitCatalog::default();
        let entries = filter_catalog_entries(
            &catalog,
            &DoodadCatalog::default(),
            DevTab::Units,
            SpawnMode::Unit,
            "wolf",
            true,
        );
        assert_eq!(entries.len(), 1);
        assert!(matches!(
            entries[0].definition,
            DefinitionId::Unit(ref id) if id.as_str() == "wolf"
        ));
    }

    #[test]
    fn catalog_filter_respects_enabled_only() {
        let mut definitions = UnitCatalog::default().definitions().to_vec();
        definitions.push(
            UnitDefinition::new(
                UnitDefinitionId::new("disabled_test"),
                "Disabled Test",
                "neutral",
                1,
                10,
                1,
                1,
                1,
                1,
                1,
                1,
                1.0,
                "T1",
                5.0,
                0.5,
                45.0,
                false,
                crate::world::UnitRenderKey::unset(),
            ),
        );
        let catalog = UnitCatalog::from_definitions(definitions).unwrap();
        let all = filter_catalog_entries(
            &catalog,
            &DoodadCatalog::default(),
            DevTab::Units,
            SpawnMode::Unit,
            "",
            false,
        );
        let enabled = filter_catalog_entries(
            &catalog,
            &DoodadCatalog::default(),
            DevTab::Units,
            SpawnMode::Unit,
            "",
            true,
        );
        assert!(all.len() > enabled.len());
        assert!(enabled.iter().all(|entry| entry.enabled));
    }

    #[test]
    fn doodad_tab_lists_doodad_definitions() {
        let catalog = DoodadCatalog::default();
        let entries = filter_catalog_entries(
            &UnitCatalog::default(),
            &catalog,
            DevTab::Doodads,
            SpawnMode::Doodad,
            "tree",
            true,
        );
        assert!(!entries.is_empty());
        assert!(entries
            .iter()
            .all(|entry| matches!(entry.definition, DefinitionId::Doodad(_))));
    }
}
