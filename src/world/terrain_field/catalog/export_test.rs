//! Export starter terrain field catalog to committed RON (run once when definitions change).

#[cfg(any(test, feature = "dev"))]
#[test]
#[ignore = "manual: writes assets/terrain_fields/catalog.ron"]
fn write_starter_terrain_field_catalog_ron() {
    use std::path::Path;

    use crate::world::{TerrainFieldCatalogRon, starter_terrain_field_definitions};

    let catalog = TerrainFieldCatalogRon {
        definitions: starter_terrain_field_definitions(),
    };
    let text = ron::ser::to_string_pretty(&catalog, ron::ser::PrettyConfig::default()).unwrap();
    std::fs::create_dir_all("assets/terrain_fields").unwrap();
    std::fs::write("assets/terrain_fields/catalog.ron", text).unwrap();
}
