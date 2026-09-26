//! CG1 appearance profile import and resolution tests.

#[cfg(all(test, feature = "data-import"))]
mod workbook_tests {
    use std::collections::BTreeMap;

    use crate::data_import::appearance::import_appearance_profiles_from_excel;
    use crate::data_import::paths::dev_design_workbook_path;
    use crate::data_import::{
        import_units_from_excel, resolve_dev_faction_catalog, resolve_dev_inventory_profile_catalog,
        resolve_dev_animation_profile_catalog, resolve_dev_species_catalog,
        resolve_dev_weapon_catalog,
    };
    use crate::dev::scenes::{SceneUnitAppearanceRecord, SceneUnitRecord, SCENE_VERSION};
    use crate::world::{
        AppearanceProfileCatalog, AppearanceProfileId, BodyVariantId, UnitDefinitionId,
        UnitOwnership, UnitSource, WorldData, create_unit, effective_unit_render_key,
        resolve_canonical_default_appearance, validate_unit_appearance,
    };
    use crate::world::{ChunkCoord, ChunkLayout, LocalPosition, UnitCatalog, WorldPosition};
    use bevy::prelude::Vec3;

    fn dev_appearance_catalog() -> AppearanceProfileCatalog {
        let path = dev_design_workbook_path();
        import_appearance_profiles_from_excel(&path)
            .expect("appearance import")
            .0
    }

    fn dev_unit_catalog(appearance: &AppearanceProfileCatalog) -> UnitCatalog {
        let path = dev_design_workbook_path();
        let factions = resolve_dev_faction_catalog();
        let species = resolve_dev_species_catalog();
        let weapons = resolve_dev_weapon_catalog();
        let animation_profiles = resolve_dev_animation_profile_catalog();
        let inventory_profiles = resolve_dev_inventory_profile_catalog();
        let (definitions, summary) = import_units_from_excel(
            &path,
            &factions,
            &species,
            &weapons,
            &animation_profiles,
            &inventory_profiles,
            appearance,
        )
        .expect("unit import");
        let catalog = UnitCatalog::from_definitions(definitions).expect("unit catalog");
        for id in ["U-0004", "U-0005"] {
            assert!(
                catalog.get(&UnitDefinitionId::new(id)).is_some(),
                "{id} missing from unit catalog; failed={} warnings={:?}",
                summary.rows_failed,
                summary.warnings,
            );
        }
        catalog
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    #[test]
    fn design_workbook_imports_human_appearance_profile() {
        if !dev_design_workbook_path().is_file() {
            return;
        }
        let (catalog, summary) =
            import_appearance_profiles_from_excel(&dev_design_workbook_path()).unwrap();
        assert_eq!(summary.rows_failed, 0, "warnings={:?}", summary.warnings);
        let human = catalog
            .get(&AppearanceProfileId::new("human"))
            .expect("human profile");
        assert!(human.enabled);
        assert_eq!(human.height_scale_default, 1.0);
        assert!(human.height_scale_min <= human.height_scale_default);
        assert!(human.height_scale_default <= human.height_scale_max);
        assert_eq!(human.enabled_parameters().len(), 9);

        let male = human
            .body_variant(&BodyVariantId::new("human_male"))
            .expect("male variant");
        assert_eq!(male.render_key.0.as_deref(), Some("human_male"));
        let female = human
            .body_variant(&BodyVariantId::new("human_female"))
            .expect("female variant");
        assert_eq!(female.render_key.0.as_deref(), Some("human_female"));
    }

    #[test]
    fn human_unit_definitions_link_appearance_profile() {
        if !dev_design_workbook_path().is_file() {
            return;
        }
        let appearance = dev_appearance_catalog();
        let catalog = dev_unit_catalog(&appearance);

        let male = catalog
            .get(&UnitDefinitionId::new("U-0004"))
            .expect("human male definition");
        assert_eq!(
            male.appearance_profile_id.as_ref().map(|id| id.as_str()),
            Some("human")
        );
        assert_eq!(
            male.default_body_variant_id.as_ref().map(|id| id.as_str()),
            Some("human_male")
        );

        let female = catalog
            .get(&UnitDefinitionId::new("U-0005"))
            .expect("human female definition");
        assert_eq!(
            female.appearance_profile_id.as_ref().map(|id| id.as_str()),
            Some("human")
        );
        assert_eq!(
            female.default_body_variant_id.as_ref().map(|id| id.as_str()),
            Some("human_female")
        );

        let male_default = resolve_canonical_default_appearance(male, &appearance).unwrap();
        assert_eq!(male_default.body_variant_id.as_str(), "human_male");
        assert_eq!(male_default.height_scale, 1.0);
        assert_eq!(male_default.morphs.len(), 9);

        let female_default = resolve_canonical_default_appearance(female, &appearance).unwrap();
        assert_eq!(female_default.body_variant_id.as_str(), "human_female");
    }

    #[test]
    fn new_unit_spawn_receives_canonical_appearance() {
        if !dev_design_workbook_path().is_file() {
            return;
        }
        let appearance = dev_appearance_catalog();
        let catalog = dev_unit_catalog(&appearance);
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let record = create_unit(
            &catalog,
            &appearance,
            &mut world,
            &UnitDefinitionId::new("U-0004"),
            pos(1.0, 1.0),
            UnitSource::Dev,
        )
        .unwrap();
        let appearance_state = record.appearance.expect("appearance on spawn");
        assert_eq!(appearance_state.profile_id.as_str(), "human");
        assert_eq!(appearance_state.body_variant_id.as_str(), "human_male");
        validate_unit_appearance(&appearance_state, catalog.get(&record.definition_id).unwrap(), &appearance)
            .unwrap();
    }

    #[test]
    fn appearance_round_trips_through_scene_record() {
        if !dev_design_workbook_path().is_file() {
            return;
        }
        let appearance = dev_appearance_catalog();
        let catalog = dev_unit_catalog(&appearance);
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let unit = create_unit(
            &catalog,
            &appearance,
            &mut world,
            &UnitDefinitionId::new("U-0005"),
            pos(2.0, 2.0),
            UnitSource::Dev,
        )
        .unwrap();
        world
            .mutate_unit(unit.id, |record| {
                if let Some(state) = &mut record.appearance {
                    state.height_scale = 1.08;
                }
            })
            .unwrap();
        let scene_unit = SceneUnitRecord::from_record(world.get_unit(unit.id).unwrap());
        assert_eq!(SCENE_VERSION, 19);
        let restored = scene_unit.to_record(&catalog, &appearance).unwrap();
        let restored_appearance = restored.appearance.expect("restored appearance");
        assert!((restored_appearance.height_scale - 1.08).abs() < 1e-4);
        assert_eq!(restored_appearance.body_variant_id.as_str(), "human_female");
    }

    #[test]
    fn legacy_scene_without_appearance_migrates_canonical_defaults_once() {
        if !dev_design_workbook_path().is_file() {
            return;
        }
        let appearance = dev_appearance_catalog();
        let catalog = dev_unit_catalog(&appearance);
        let scene_unit = SceneUnitRecord {
            id: 42,
            definition_id: "U-0004".into(),
            position: crate::dev::scenes::SceneWorldPosition::from_world(pos(0.0, 0.0)),
            rotation: crate::dev::scenes::SceneQuat::from_quat(bevy::prelude::Quat::IDENTITY),
            state: crate::dev::scenes::SceneUnitState::Idle,
            source: crate::dev::scenes::SceneUnitSource::Authored,
            owner_id: None,
            team_id: None,
            affiliation: None,
            current_space_id: 0,
            inventory_id: None,
            equipment: None,
            faction_id: None,
            species_id: None,
            settlement_id: None,
            current_nutrition: None,
            work_skill_overrides: Vec::new(),
            appearance: None,
            dialogue: None,
        };
        let record = scene_unit.to_record(&catalog, &appearance).unwrap();
        let migrated = record.appearance.expect("legacy migration appearance");
        assert_eq!(migrated.profile_id.as_str(), "human");
        assert_eq!(migrated.body_variant_id.as_str(), "human_male");
        assert_eq!(migrated.morphs.len(), 9);
    }

    #[test]
    fn height_scale_composes_with_definition_baseline_only_for_presentation() {
        if !dev_design_workbook_path().is_file() {
            return;
        }
        let appearance = dev_appearance_catalog();
        let catalog = dev_unit_catalog(&appearance);
        let definition = catalog
            .get(&UnitDefinitionId::new("U-0004"))
            .expect("human male");
        let baseline = crate::world::unit_definition_visual_scale(definition);
        let short = crate::world::unit_visual_scale(definition, 0.9);
        let tall = crate::world::unit_visual_scale(definition, 1.1);
        assert!((short.x - baseline.x * 0.9).abs() < 1e-4);
        assert!((tall.x - baseline.x * 1.1).abs() < 1e-4);
        assert_ne!(short, tall);
    }

    #[test]
    fn effective_render_key_uses_appearance_body_variant() {
        if !dev_design_workbook_path().is_file() {
            return;
        }
        let appearance = dev_appearance_catalog();
        let catalog = dev_unit_catalog(&appearance);
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let male = create_unit(
            &catalog,
            &appearance,
            &mut world,
            &UnitDefinitionId::new("U-0004"),
            pos(1.0, 1.0),
            UnitSource::Dev,
        )
        .unwrap();
        let female = create_unit(
            &catalog,
            &appearance,
            &mut world,
            &UnitDefinitionId::new("U-0005"),
            pos(2.0, 2.0),
            UnitSource::Dev,
        )
        .unwrap();
        let male_def = catalog.get(&male.definition_id).unwrap();
        let female_def = catalog.get(&female.definition_id).unwrap();
        let male_key = effective_unit_render_key(&male, male_def, &appearance)
            .unwrap()
            .0
            .unwrap();
        let female_key = effective_unit_render_key(&female, female_def, &appearance)
            .unwrap()
            .0
            .unwrap();
        assert_eq!(male_key, "human_male");
        assert_eq!(female_key, "human_female");
        assert_ne!(male_key, female_key);
    }

    #[test]
    fn invalid_appearance_values_fail_explicitly() {
        if !dev_design_workbook_path().is_file() {
            return;
        }
        let appearance = dev_appearance_catalog();
        let catalog = dev_unit_catalog(&appearance);
        let definition = catalog
            .get(&UnitDefinitionId::new("U-0004"))
            .expect("human male");
        let mut bad = resolve_canonical_default_appearance(definition, &appearance).unwrap();
        bad.height_scale = 99.0;
        assert!(validate_unit_appearance(&bad, definition, &appearance).is_err());

        bad = resolve_canonical_default_appearance(definition, &appearance).unwrap();
        bad.morphs.remove(&crate::world::AppearanceParamId::new("build"));
        assert!(validate_unit_appearance(&bad, definition, &appearance).is_err());

        let scene = SceneUnitAppearanceRecord {
            profile_id: "missing".into(),
            body_variant_id: "human_male".into(),
            height_scale: 1.0,
            morphs: Vec::new(),
            generation_seed: None,
        };
        assert!(scene.to_unit(&appearance).is_err());
    }

    #[test]
    fn empty_appearance_catalog_has_no_fallback_profiles() {
        let catalog = AppearanceProfileCatalog::empty();
        assert!(catalog.is_empty());
        assert!(catalog.get(&AppearanceProfileId::new("human")).is_none());
    }
}

#[cfg(all(test, feature = "data-import"))]
mod schema_tests {
    use crate::data_import::appearance::schema::{
        AppearanceBodyVariantImportRow, AppearanceParameterImportRow, AppearanceProfileImportRow,
        assemble_profiles,
    };

    fn sample_profile_row() -> AppearanceProfileImportRow {
        AppearanceProfileImportRow {
            row_number: 2,
            profile_id: "human".into(),
            species_id: "human".into(),
            enabled: true,
            schema_version: 1,
            height_min: 0.85,
            height_max: 1.15,
            height_default: 1.0,
        }
    }

    #[test]
    fn assemble_rejects_duplicate_parameters() {
        let profile = sample_profile_row();
        let variants = vec![AppearanceBodyVariantImportRow {
            row_number: 2,
            profile_id: "human".into(),
            variant_id: "human_male".into(),
            render_key: "human_male".into(),
            display_name: "Male".into(),
            enabled: true,
        }];
        let parameters = vec![
            AppearanceParameterImportRow {
                row_number: 2,
                profile_id: "human".into(),
                param_id: "build".into(),
                display_name: "Build".into(),
                category: "Body".into(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                display_order: 1,
                enabled: true,
            },
            AppearanceParameterImportRow {
                row_number: 3,
                profile_id: "human".into(),
                param_id: "build".into(),
                display_name: "Build".into(),
                category: "Body".into(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                display_order: 2,
                enabled: true,
            },
        ];
        let err = assemble_profiles(vec![profile], variants, parameters, Vec::new())
            .unwrap_err();
        assert!(err.contains("duplicate parameter"));
    }

    #[test]
    fn assemble_rejects_default_outside_range() {
        let profile = sample_profile_row();
        let variants = vec![AppearanceBodyVariantImportRow {
            row_number: 2,
            profile_id: "human".into(),
            variant_id: "human_male".into(),
            render_key: "human_male".into(),
            display_name: "Male".into(),
            enabled: true,
        }];
        let parameters = vec![AppearanceParameterImportRow {
            row_number: 2,
            profile_id: "human".into(),
            param_id: "build".into(),
            display_name: "Build".into(),
            category: "Body".into(),
            min: 0.0,
            max: 1.0,
            default: 1.5,
            display_order: 1,
            enabled: true,
        }];
        let err = assemble_profiles(vec![profile], variants, parameters, Vec::new())
            .unwrap_err();
        assert!(err.contains("Default"));
    }
}
