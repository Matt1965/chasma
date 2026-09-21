use super::*;

fn dev_catalogs() -> (
    crate::world::UnitCatalog,
    crate::world::AppearanceProfileCatalog,
) {
    let appearance_profiles = crate::data_import::resolve_dev_appearance_profile_catalog();
    let factions = crate::data_import::resolve_dev_faction_catalog();
    let species = crate::data_import::resolve_dev_species_catalog();
    let weapons = crate::data_import::resolve_dev_weapon_catalog();
    let animation_profiles = crate::data_import::resolve_dev_animation_profile_catalog();
    let inventory_profiles = crate::data_import::resolve_dev_inventory_profile_catalog();
    let units = crate::data_import::resolve_dev_unit_catalog(
        &factions,
        &species,
        &weapons,
        &animation_profiles,
        &inventory_profiles,
        &appearance_profiles,
        None,
    );
    (units, appearance_profiles)
}

#[test]
fn starter_catalog_has_distinct_origins() {
    let (units, profiles) = dev_catalogs();
    let catalog = seed_origin_catalog(&units, &profiles);
    assert_eq!(catalog.definitions().len(), 2);
    let exile = catalog.get(&OriginId::new("exile_pair")).unwrap();
    let lone = catalog.get(&OriginId::new("lone_survivor")).unwrap();
    assert_eq!(exile.member_count(), 2);
    assert_eq!(lone.member_count(), 1);
    assert_ne!(exile.id, lone.id);
}

#[test]
fn snapshot_members_keep_independent_definition_ids() {
    let (units, profiles) = dev_catalogs();
    let catalog = seed_origin_catalog(&units, &profiles);
    let exile = catalog.get(&OriginId::new("exile_pair")).unwrap();
    assert_ne!(
        exile.members[0].definition_id.as_str(),
        exile.members[1].definition_id.as_str()
    );
}
