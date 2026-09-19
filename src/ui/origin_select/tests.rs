use crate::menu::build_starting_squad_draft;
use crate::units::presentation::roster_preview_offsets;
use crate::world::{OriginCatalog, OriginId, starter_origin_catalog};

#[test]
fn roster_preview_offsets_center_pair() {
    let offsets = roster_preview_offsets(2, 1.5);
    assert_eq!(offsets.len(), 2);
    assert!(offsets[0].x < 0.0);
    assert!(offsets[1].x > 0.0);
}

#[test]
fn starter_catalog_is_valid_resource() {
    let _: OriginCatalog = starter_origin_catalog();
}

#[test]
fn build_draft_for_lone_survivor_has_one_member() {
    let origins = starter_origin_catalog();
    let appearance_profiles = crate::data_import::resolve_dev_appearance_profile_catalog();
    let units = crate::data_import::resolve_dev_unit_catalog(
        &crate::data_import::resolve_dev_faction_catalog(),
        &crate::data_import::resolve_dev_species_catalog(),
        &crate::data_import::resolve_dev_weapon_catalog(),
        &crate::data_import::resolve_dev_animation_profile_catalog(),
        &crate::data_import::resolve_dev_inventory_profile_catalog(),
        &appearance_profiles,
        None,
    );
    let draft = build_starting_squad_draft(
        &OriginId::new("lone_survivor"),
        &origins,
        &units,
        &appearance_profiles,
    )
    .unwrap();
    assert_eq!(draft.members.len(), 1);
}
