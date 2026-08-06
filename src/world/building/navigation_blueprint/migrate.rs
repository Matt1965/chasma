//! Schema migration for building navigation blueprints (NV2).

use super::definition::{
    BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION, BuildingNavigationBlueprint,
    NavigationFloorDefinition, NavigationRegionDefinition,
};
use super::error::BuildingNavigationBlueprintError;

/// Summary of a migration run (not persisted).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlueprintMigrationReport {
    pub original_schema_version: u32,
    pub final_schema_version: u32,
    pub floors_migrated: u32,
    pub entrance_refs_populated: u32,
    pub transition_from_refs_populated: u32,
    pub transition_to_refs_populated: u32,
    pub changed: bool,
}

/// Migrate a blueprint to the current schema in place.
pub fn migrate_blueprint_to_current(
    blueprint: &mut BuildingNavigationBlueprint,
) -> Result<BlueprintMigrationReport, BuildingNavigationBlueprintError> {
    let original_schema_version = blueprint.schema_version;
    let mut report = BlueprintMigrationReport {
        original_schema_version,
        final_schema_version: BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION,
        ..Default::default()
    };

    if is_current_schema(blueprint) {
        blueprint.validate()?;
        return Ok(report);
    }

    let blueprint_id = blueprint.id.clone();
    for index in 0..blueprint.floors.len() {
        migrate_floor(&blueprint_id, &mut blueprint.floors[index], &mut report)?;
    }

    for index in 0..blueprint.entrances.len() {
        if blueprint.entrances[index].region_key.is_some() {
            continue;
        }
        let floor_key = blueprint.entrances[index].floor_key.clone();
        let feature_key = blueprint.entrances[index].key.clone();
        let floor = blueprint.floor_by_key(&floor_key).ok_or_else(|| {
            BuildingNavigationBlueprintError::FloorKeyMissing {
                blueprint_id: blueprint.id.clone(),
                floor_key,
            }
        })?;
        let region_key = sole_region_key_or_error(&blueprint_id, floor, &feature_key)?;
        blueprint.entrances[index].region_key = Some(region_key);
        report.entrance_refs_populated += 1;
        report.changed = true;
    }

    for index in 0..blueprint.vertical_transitions.len() {
        let transition_key = blueprint.vertical_transitions[index].key.clone();
        if blueprint.vertical_transitions[index]
            .from_region_key
            .is_none()
        {
            let floor_key = blueprint.vertical_transitions[index].from_floor_key.clone();
            let floor = blueprint.floor_by_key(&floor_key).ok_or_else(|| {
                BuildingNavigationBlueprintError::FloorKeyMissing {
                    blueprint_id: blueprint.id.clone(),
                    floor_key,
                }
            })?;
            let region_key = sole_region_key_or_error(&blueprint_id, floor, &transition_key)?;
            blueprint.vertical_transitions[index].from_region_key = Some(region_key);
            report.transition_from_refs_populated += 1;
            report.changed = true;
        }
        if blueprint.vertical_transitions[index]
            .to_region_key
            .is_none()
        {
            let floor_key = blueprint.vertical_transitions[index].to_floor_key.clone();
            let floor = blueprint.floor_by_key(&floor_key).ok_or_else(|| {
                BuildingNavigationBlueprintError::FloorKeyMissing {
                    blueprint_id: blueprint.id.clone(),
                    floor_key,
                }
            })?;
            let region_key = sole_region_key_or_error(&blueprint_id, floor, &transition_key)?;
            blueprint.vertical_transitions[index].to_region_key = Some(region_key);
            report.transition_to_refs_populated += 1;
            report.changed = true;
        }
    }

    blueprint.schema_version = BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION;
    if blueprint.schema_version != original_schema_version {
        report.changed = true;
    }

    blueprint.validate()?;
    report.final_schema_version = blueprint.schema_version;
    Ok(report)
}

fn is_current_schema(blueprint: &BuildingNavigationBlueprint) -> bool {
    blueprint.schema_version == BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION
        && blueprint
            .floors
            .iter()
            .all(|floor| !floor.regions.is_empty() && floor.walkable_outline_legacy.is_none())
}

fn migrate_floor(
    blueprint_id: &crate::world::building::navigation_blueprint::id::BuildingNavigationBlueprintId,
    floor: &mut NavigationFloorDefinition,
    report: &mut BlueprintMigrationReport,
) -> Result<(), BuildingNavigationBlueprintError> {
    let has_legacy = floor.walkable_outline_legacy.is_some();
    let has_regions = !floor.regions.is_empty();

    match (has_legacy, has_regions) {
        (true, true) => Err(BuildingNavigationBlueprintError::AmbiguousFloorGeometry {
            blueprint_id: blueprint_id.clone(),
            floor_key: floor.key.clone(),
        }),
        (false, false) => Err(BuildingNavigationBlueprintError::FloorHasNoRegions {
            blueprint_id: blueprint_id.clone(),
            floor_key: floor.key.clone(),
        }),
        (true, false) => {
            let legacy = floor.walkable_outline_legacy.take().expect("checked above");
            floor.regions.push(NavigationRegionDefinition {
                key: "main".to_string(),
                display_label: floor.display_label.clone(),
                room_tag: floor.room_tag.clone(),
                walkable_outline: legacy,
            });
            report.floors_migrated += 1;
            report.changed = true;
            Ok(())
        }
        (false, true) => Ok(()),
    }
}

fn sole_region_key_or_error(
    blueprint_id: &crate::world::building::navigation_blueprint::id::BuildingNavigationBlueprintId,
    floor: &NavigationFloorDefinition,
    feature_key: &str,
) -> Result<String, BuildingNavigationBlueprintError> {
    floor
        .single_region_key()
        .map(str::to_string)
        .ok_or_else(
            || BuildingNavigationBlueprintError::RegionReferenceAmbiguous {
                blueprint_id: blueprint_id.clone(),
                feature_key: feature_key.to_string(),
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::catalog::BuildingDefinitionId;
    use crate::world::building::navigation_blueprint::catalog::BuildingNavigationBlueprintCatalogRon;
    use crate::world::building::navigation_blueprint::definition::{
        BuildingNavigationBlueprintInstanceOverride, NavigationEntranceDefinition,
        NavigationFloorDefinition, NavigationPolygon2d, NavigationVerticalTransitionDefinition,
        NavigationVerticalTransitionKind,
    };
    use crate::world::building::navigation_blueprint::resolve::resolve_building_navigation_blueprint;
    use crate::world::building::navigation_blueprint::{
        BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH, BuildingNavigationBlueprintCatalog,
    };
    use crate::world::starter_building_definitions;

    fn v1_floor() -> NavigationFloorDefinition {
        NavigationFloorDefinition {
            floor_id: 0,
            key: "ground".to_string(),
            display_label: "Ground".to_string(),
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: Some("hall".to_string()),
            walkable_outline_legacy: Some(NavigationPolygon2d::rectangle(4.0, 4.0)),
            regions: Vec::new(),
        }
    }

    fn v1_blueprint() -> BuildingNavigationBlueprint {
        BuildingNavigationBlueprint {
            id: "test_hut".into(),
            display_name: "Test Hut".to_string(),
            schema_version: 1,
            metadata: Default::default(),
            floors: vec![v1_floor()],
            entrances: vec![NavigationEntranceDefinition {
                key: "exterior_entrance".to_string(),
                floor_key: "ground".to_string(),
                region_key: None,
                local_position_xz: [2.0, 0.0],
                radius_meters: 1.5,
                interior_spawn_local: [2.0, 0.0, 1.0],
                bidirectional: true,
                door_key: None,
            }],
            vertical_transitions: Vec::new(),
            region_connections: Vec::new(),
            enabled: true,
        }
    }

    #[test]
    fn schema_v1_ron_deserializes_unchanged() {
        let text = r#"
(
    id: ("test_hut"),
    display_name: "Test Hut",
    schema_version: 1,
    metadata: (
        source_render_key: None,
        generation_revision: None,
        tags: [],
        extensions: {},
    ),
    floors: [
        (
            floor_id: 0,
            key: "ground",
            display_label: "Ground",
            elevation_meters: 0.0,
            visibility_group_id: 1,
            room_tag: Some("hall"),
            walkable_outline: (
                vertices_xz: [
                    (0.0, 0.0),
                    (4.0, 0.0),
                    (4.0, 4.0),
                    (0.0, 4.0),
                ],
            ),
        ),
    ],
    entrances: [],
    vertical_transitions: [],
    enabled: true,
)
"#;
        let blueprint: BuildingNavigationBlueprint = ron::from_str(text).expect("deserialize");
        assert_eq!(blueprint.schema_version, 1);
        assert!(blueprint.floors[0].walkable_outline_legacy.is_some());
        assert!(blueprint.floors[0].regions.is_empty());
    }

    #[test]
    fn v1_floor_becomes_main_region() {
        let mut blueprint = v1_blueprint();
        let report = migrate_blueprint_to_current(&mut blueprint).expect("migrate");
        assert_eq!(report.floors_migrated, 1);
        assert_eq!(blueprint.schema_version, 2);
        assert!(blueprint.floors[0].walkable_outline_legacy.is_none());
        assert_eq!(blueprint.floors[0].regions.len(), 1);
        assert_eq!(blueprint.floors[0].regions[0].key, "main");
        assert_eq!(
            blueprint.floors[0].regions[0].room_tag,
            Some("hall".to_string())
        );
    }

    #[test]
    fn entrance_and_transition_region_keys_populated() {
        let mut blueprint = v1_blueprint();
        blueprint.floors.push(NavigationFloorDefinition {
            floor_id: 1,
            key: "upper".to_string(),
            display_label: "Upper".to_string(),
            elevation_meters: 4.0,
            visibility_group_id: 2,
            room_tag: None,
            walkable_outline_legacy: Some(NavigationPolygon2d::rectangle(4.0, 4.0)),
            regions: Vec::new(),
        });
        blueprint
            .vertical_transitions
            .push(NavigationVerticalTransitionDefinition {
                key: "stairs".to_string(),
                kind: NavigationVerticalTransitionKind::Stair,
                from_floor_key: "ground".to_string(),
                to_floor_key: "upper".to_string(),
                from_region_key: None,
                to_region_key: None,
                from_local_position_xz: [3.0, 3.0],
                from_radius_meters: 1.25,
                to_local_position: [3.0, 4.0, 3.0],
                bidirectional: true,
            });
        let report = migrate_blueprint_to_current(&mut blueprint).expect("migrate");
        assert_eq!(report.entrance_refs_populated, 1);
        assert_eq!(report.transition_from_refs_populated, 1);
        assert_eq!(report.transition_to_refs_populated, 1);
        assert_eq!(blueprint.entrances[0].region_key, Some("main".to_string()));
        assert_eq!(
            blueprint.vertical_transitions[0].from_region_key,
            Some("main".to_string())
        );
        assert_eq!(
            blueprint.vertical_transitions[0].to_region_key,
            Some("main".to_string())
        );
    }

    #[test]
    fn migration_is_idempotent() {
        let mut blueprint = v1_blueprint();
        migrate_blueprint_to_current(&mut blueprint).expect("first migrate");
        let report = migrate_blueprint_to_current(&mut blueprint).expect("second migrate");
        assert!(!report.changed);
        assert_eq!(report.floors_migrated, 0);
    }

    #[test]
    fn ambiguous_floor_geometry_rejected() {
        let mut blueprint = v1_blueprint();
        blueprint.floors[0]
            .regions
            .push(NavigationRegionDefinition {
                key: "extra".to_string(),
                display_label: "Extra".to_string(),
                room_tag: None,
                walkable_outline: NavigationPolygon2d::rectangle(2.0, 2.0),
            });
        assert!(matches!(
            migrate_blueprint_to_current(&mut blueprint),
            Err(BuildingNavigationBlueprintError::AmbiguousFloorGeometry { .. })
        ));
    }

    #[test]
    fn floor_without_geometry_rejected() {
        let mut blueprint = v1_blueprint();
        blueprint.floors[0].walkable_outline_legacy = None;
        assert!(matches!(
            migrate_blueprint_to_current(&mut blueprint),
            Err(BuildingNavigationBlueprintError::FloorHasNoRegions { .. })
        ));
    }

    #[test]
    fn missing_region_reference_on_multi_region_floor_rejected() {
        let mut blueprint = v1_blueprint();
        migrate_blueprint_to_current(&mut blueprint).expect("initial migrate");
        blueprint.floors[0]
            .regions
            .push(NavigationRegionDefinition {
                key: "east".to_string(),
                display_label: "East".to_string(),
                room_tag: None,
                walkable_outline: NavigationPolygon2d::rectangle(2.0, 2.0),
            });
        blueprint.entrances[0].region_key = None;
        assert!(matches!(
            migrate_blueprint_to_current(&mut blueprint),
            Err(BuildingNavigationBlueprintError::RegionReferenceAmbiguous { .. })
        ));
    }

    #[test]
    fn migrated_ron_is_canonical_schema_v2() {
        let mut blueprint = v1_blueprint();
        migrate_blueprint_to_current(&mut blueprint).expect("migrate");
        let text = ron::ser::to_string_pretty(&blueprint, ron::ser::PrettyConfig::default())
            .expect("serialize");
        assert!(text.contains("schema_version: 2"));
        assert!(text.contains("regions:"));
        assert!(!text.contains("walkable_outline_legacy"));
        assert!(blueprint.floors[0].walkable_outline_legacy.is_none());
        assert!(text.contains("walkable_outline:"));
    }

    #[test]
    fn inline_instance_override_migrates() {
        let mut inline = v1_blueprint();
        migrate_blueprint_to_current(&mut inline).expect("migrate inline");
        let catalog = BuildingNavigationBlueprintCatalog::default();
        let definition = starter_building_definitions()
            .into_iter()
            .find(|def| def.id == BuildingDefinitionId::new("hut"))
            .expect("hut");
        let resolved = resolve_building_navigation_blueprint(
            &definition,
            &catalog,
            Some(&BuildingNavigationBlueprintInstanceOverride::inline(inline)),
        )
        .expect("resolve")
        .expect("blueprint");
        assert_eq!(resolved.blueprint().schema_version, 2);
        assert_eq!(
            resolved.blueprint().entrances[0].region_key,
            Some("main".to_string())
        );
    }

    #[test]
    fn catalog_definitions_load_migrate_and_validate() {
        let text = std::fs::read_to_string(BUILDING_NAVIGATION_BLUEPRINT_CATALOG_RON_PATH)
            .expect("read catalog");
        let file: BuildingNavigationBlueprintCatalogRon =
            ron::from_str(&text).expect("parse catalog ron");
        for mut definition in file.definitions {
            let id = definition.id.clone();
            // No entry may be exempt: the real loader rejects the whole catalog when one
            // definition fails to migrate or validate.
            migrate_blueprint_to_current(&mut definition)
                .unwrap_or_else(|err| panic!("migrate {id}: {err}"));
            assert_eq!(definition.schema_version, 2);
            for floor in &definition.floors {
                assert!(!floor.regions.is_empty());
                assert!(floor.walkable_outline_legacy.is_none());
            }
            definition.validate().expect("validate");
        }
    }
}
