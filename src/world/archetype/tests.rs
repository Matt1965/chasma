use super::*;
use crate::world::equipment::EquipmentSlot;
use crate::world::relationship::SpeciesId;
use crate::world::Affiliation;
use crate::world::{
    BuildingDefinitionId, BuildingLifecycleState, ItemCatalog, ItemCategoryCatalog,
    UnitCatalog, UnitDefinitionId, starter_item_category_definitions,
    starter_unit_definitions, test_equipment_fixture_definitions,
};

fn unit_catalog() -> UnitCatalog {
    UnitCatalog::from_definitions(starter_unit_definitions()).unwrap()
}

fn item_catalog() -> ItemCatalog {
    let categories =
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
    let mut items = test_equipment_fixture_definitions();
    items.extend(crate::world::starter_item_definitions());
    ItemCatalog::from_definitions(items, &categories).unwrap()
}

fn sample_unit_archetypes() -> UnitArchetypeCatalog {
    UnitArchetypeCatalog::from_definitions(vec![
        UnitArchetypeDefinition {
            id: UnitArchetypeId::new("bandit_loadout"),
            display_name: "Bandit".to_string(),
            applicable_species: vec![SpeciesId::new("human")],
            gold_min: 5,
            gold_max: 25,
            affiliation_override: Some(Affiliation::Hostile),
            equipment: vec![ArchetypeEquipmentEntry {
                item_id: crate::world::ItemDefinitionId::new("iron_sword"),
                slot: EquipmentSlot::Weapon,
            }],
            inventory_stacks: Vec::new(),
            dialogue: None,
            enabled: true,
        },
        UnitArchetypeDefinition {
            id: UnitArchetypeId::new("guard_loadout"),
            display_name: "Guard".to_string(),
            applicable_species: vec![SpeciesId::new("human")],
            gold_min: 0,
            gold_max: 0,
            affiliation_override: Some(Affiliation::Player),
            equipment: Vec::new(),
            inventory_stacks: Vec::new(),
            dialogue: None,
            enabled: true,
        },
    ])
    .unwrap()
}

#[test]
fn unit_archetype_catalog_rejects_duplicate_ids() {
    let defs = vec![
        UnitArchetypeDefinition {
            id: UnitArchetypeId::new("dup"),
            display_name: "A".to_string(),
            applicable_species: vec![SpeciesId::new("human")],
            gold_min: 0,
            gold_max: 0,
            affiliation_override: None,
            equipment: Vec::new(),
            inventory_stacks: Vec::new(),
            dialogue: None,
            enabled: true,
        },
        UnitArchetypeDefinition {
            id: UnitArchetypeId::new("dup"),
            display_name: "B".to_string(),
            applicable_species: vec![SpeciesId::new("human")],
            gold_min: 0,
            gold_max: 0,
            affiliation_override: None,
            equipment: Vec::new(),
            inventory_stacks: Vec::new(),
            dialogue: None,
            enabled: true,
        },
    ];
    assert!(matches!(
        UnitArchetypeCatalog::from_definitions(defs),
        Err(UnitArchetypeCatalogError::DuplicateId(_))
    ));
}

#[test]
fn unit_archetype_rejects_invalid_gold_range() {
    let defs = vec![UnitArchetypeDefinition {
        id: UnitArchetypeId::new("bad_gold"),
        display_name: "Bad".to_string(),
        applicable_species: vec![SpeciesId::new("human")],
        gold_min: 10,
        gold_max: 5,
        affiliation_override: None,
        equipment: Vec::new(),
        inventory_stacks: Vec::new(),
        dialogue: None,
        enabled: true,
    }];
    assert!(matches!(
        UnitArchetypeCatalog::from_definitions(defs),
        Err(UnitArchetypeCatalogError::InvalidDefinition { .. })
    ));
}

#[test]
fn unit_archetype_filters_by_species() {
    let catalog = unit_catalog();
    let archetypes = sample_unit_archetypes();
    let bandit = UnitDefinitionId::new("bandit");
    let wolf = UnitDefinitionId::new("wolf");

    assert_eq!(
        archetypes.archetypes_for_unit(&bandit, &catalog, true).len(),
        2
    );
    assert_eq!(
        archetypes.archetypes_for_unit(&wolf, &catalog, true).len(),
        0
    );
}

#[test]
fn resolve_unit_spawn_spec_default_uses_dev_affiliation() {
    let unit_catalog = unit_catalog();
    let archetypes = UnitArchetypeCatalog::default();
    let bandit = UnitDefinitionId::new("bandit");

    let spec = resolve_unit_spawn_spec(
        &bandit,
        None,
        Affiliation::Wildlife,
        &unit_catalog,
        &archetypes,
    )
    .unwrap();

    assert_eq!(spec.ownership.affiliation, Affiliation::Wildlife);
    assert!(spec.equipment.is_empty());
    assert_eq!(spec.gold_min, 0);
}

#[test]
fn resolve_unit_spawn_spec_applies_archetype_overlay() {
    let unit_catalog = unit_catalog();
    let archetypes = sample_unit_archetypes();
    let bandit = UnitDefinitionId::new("bandit");

    let spec = resolve_unit_spawn_spec(
        &bandit,
        Some(&UnitArchetypeId::new("bandit_loadout")),
        Affiliation::Wildlife,
        &unit_catalog,
        &archetypes,
    )
    .unwrap();

    assert_eq!(spec.ownership.affiliation, Affiliation::Hostile);
    assert_eq!(spec.equipment.len(), 1);
    assert_eq!(spec.gold_min, 5);
    assert_eq!(spec.gold_max, 25);
}

#[test]
fn resolve_unit_spawn_spec_rejects_inapplicable_species() {
    let unit_catalog = unit_catalog();
    let archetypes = sample_unit_archetypes();
    let wolf = UnitDefinitionId::new("wolf");

    let err = resolve_unit_spawn_spec(
        &wolf,
        Some(&UnitArchetypeId::new("guard_loadout")),
        Affiliation::Player,
        &unit_catalog,
        &archetypes,
    )
    .unwrap_err();

    assert!(matches!(
        err,
        ArchetypeResolveError::ArchetypeNotApplicable { .. }
    ));
}

#[test]
fn building_archetype_applies_only_to_base_type() {
    let building_catalog = crate::world::BuildingCatalog::default();
    let archetypes = BuildingArchetypeCatalog::from_definitions(vec![
        BuildingArchetypeDefinition {
            id: BuildingArchetypeId::new("hostile_hut"),
            display_name: "Hostile Hut".to_string(),
            base_building_id: BuildingDefinitionId::new("hut"),
            snapshot: BuildingArchetypeSnapshot {
                affiliation: Affiliation::Hostile,
                team_id: None,
                owner_id: None,
                lifecycle_state: BuildingLifecycleState::Complete,
                container_locked: false,
                uniform_scale: 1.0,
                placement_yaw_deg: 0.0,
                extensions: Default::default(),
            },
            capture_metadata: Default::default(),
            members: Vec::new(),
            enabled: true,
        },
    ])
    .unwrap();

    let hut = BuildingDefinitionId::new("hut");
    if building_catalog.get(&hut).is_some() {
        let spec = resolve_building_spawn_spec(
            &hut,
            Some(&BuildingArchetypeId::new("hostile_hut")),
            Affiliation::Player,
            &building_catalog,
            &archetypes,
        )
        .unwrap();
        assert_eq!(spec.ownership.affiliation, Affiliation::Hostile);
    }
}

#[test]
fn building_archetype_ron_roundtrip_with_members() {
    use super::building::{
        BuildingArchetypeCaptureMetadata, BuildingArchetypeLocalPose, BuildingArchetypeMember,
        BuildingArchetypeMemberKind,
    };

    let catalog = BuildingArchetypeCatalog::from_definitions(vec![
        BuildingArchetypeDefinition {
            id: BuildingArchetypeId::new("shop"),
            display_name: "Shop".to_string(),
            base_building_id: BuildingDefinitionId::new("hut"),
            snapshot: BuildingArchetypeSnapshot {
                affiliation: Affiliation::Player,
                team_id: None,
                owner_id: None,
                lifecycle_state: BuildingLifecycleState::Complete,
                container_locked: false,
                uniform_scale: 1.0,
                placement_yaw_deg: 0.0,
                extensions: Default::default(),
            },
            capture_metadata: BuildingArchetypeCaptureMetadata {
                capture_margin_meters: 3.0,
            },
            members: vec![BuildingArchetypeMember {
                kind: BuildingArchetypeMemberKind::Building,
                definition_id: "storage_chest".to_string(),
                local_pose: BuildingArchetypeLocalPose {
                    local_position: [2.0, 0.0, 0.0],
                    local_rotation: [0.0, 0.0, 0.0, 1.0],
                    uniform_scale_milli: 1000,
                    scale_x_milli: 0,
                    scale_y_milli: 0,
                    scale_z_milli: 0,
                },
                building_state: None,
                world_item_state: None,
            }],
            enabled: true,
        },
    ])
    .unwrap();
    let dir = std::env::temp_dir().join("chasma_building_archetype_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("building_archetypes.ron");
    save_building_archetype_catalog_to_ron(&catalog, &path).unwrap();
    let loaded = load_building_archetype_catalog_from_ron(&path).unwrap();
    assert_eq!(loaded.definitions().len(), 1);
    assert_eq!(loaded.definitions()[0].members.len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn building_archetype_legacy_root_only_ron_loads() {
    let dir = std::env::temp_dir().join("chasma_building_archetype_legacy");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("building_archetypes.ron");
    std::fs::write(
        &path,
        r#"(definitions: [
    (
        id: ("legacy"),
        display_name: "Legacy",
        base_building_id: ("hut"),
        snapshot: (
            affiliation: Player,
            lifecycle_state: Complete,
            container_locked: false,
            uniform_scale: 1.0,
            placement_yaw_deg: 0.0,
        ),
        enabled: true,
    ),
])"#,
    )
    .unwrap();
    let loaded = load_building_archetype_catalog_from_ron(&path).unwrap();
    assert_eq!(loaded.definitions().len(), 1);
    assert!(loaded.definitions()[0].members.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn unit_archetype_ron_roundtrip() {
    let catalog = sample_unit_archetypes();
    let dir = std::env::temp_dir().join("chasma_archetype_test");
    let path = dir.join("unit_archetypes.ron");
    save_unit_archetype_catalog_to_ron(&catalog, &path).unwrap();
    let loaded = load_unit_archetype_catalog_from_ron(&path).unwrap();
    assert_eq!(loaded.definitions().len(), catalog.definitions().len());
    let _ = std::fs::remove_dir_all(dir);
}
