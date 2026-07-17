//! Export starter source profiles to committed RON (run when definitions change).

#[cfg(any(test, feature = "dev"))]
#[test]
#[ignore = "manual: writes assets/terrain_fields/source_profiles.ron"]
fn write_starter_source_profiles_ron() {
    use std::path::Path;

    use crate::world::{TerrainFieldSourceProfileCatalogRon, starter_source_profiles};

    let catalog = TerrainFieldSourceProfileCatalogRon {
        profiles: starter_source_profiles(),
    };
    let text = ron::ser::to_string_pretty(&catalog, ron::ser::PrettyConfig::default()).unwrap();
    std::fs::create_dir_all("assets/terrain_fields").unwrap();
    std::fs::write("assets/terrain_fields/source_profiles.ron", text).unwrap();
    let _ = Path::new("assets/terrain_fields/source_profiles.ron");
}
