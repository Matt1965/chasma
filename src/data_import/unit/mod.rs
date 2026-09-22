//! Unit definition Excel import (ADR-027 U1).

#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
mod excel;
#[cfg(all(test, feature = "data-import"))]
#[cfg(test)]
mod human_glb_tests;
mod schema;
mod validate;

pub use schema::{
    DEFAULT_COLLISION_RADIUS_METERS, DEFAULT_MAX_SLOPE_DEGREES, DEFAULT_MOVE_SPEED_MPS,
    IGNORED_COLUMNS, OPTIONAL_COLUMNS, REQUIRED_COLUMNS, UnitImportRow,
    normalize_file_path_to_render_key,
};

#[cfg(feature = "data-import")]
pub use dev_load::resolve_dev_unit_catalog;
#[cfg(feature = "data-import")]
pub use excel::UNITS_SHEET_NAME;

#[cfg(feature = "data-import")]
pub fn import_units_from_excel(
    path: &std::path::Path,
    factions: &crate::world::FactionCatalog,
    species: &crate::world::SpeciesCatalog,
    weapons: &crate::world::WeaponCatalog,
    animation_profiles: &crate::world::AnimationProfileCatalog,
    inventory_profiles: &crate::world::InventoryProfileCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
) -> Result<
    (
        Vec<crate::world::UnitDefinition>,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    use std::collections::HashMap;

    use crate::world::UnitDefinitionId;

    use excel::read_unit_rows;
    use validate::validate_row;

    let rows = read_unit_rows(path)?;
    let mut summary = crate::data_import::ImportSummary {
        rows_processed: rows.len(),
        ..crate::data_import::ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_ids: HashMap<UnitDefinitionId, usize> = HashMap::new();

    for row_result in rows {
        let row = match row_result {
            Ok(row) => row,
            Err(row_err) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {}", row_err.row_number, row_err.message));
                continue;
            }
        };

        if let Err(row_err) = validate_row(&row) {
            summary.rows_failed += 1;
            summary
                .warnings
                .push(format!("row {}: {}", row_err.row_number, row_err.message));
            continue;
        }

        if !row.enabled {
            summary.warnings.push(format!(
                "row {}: Enabled=false — definition excluded from catalog",
                row.row_number
            ));
            continue;
        }

        let mut definition = match row.to_definition(factions, species) {
            Ok(definition) => definition,
            Err(message) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {message}", row.row_number));
                continue;
            }
        };

        if let Err(err) = weapons.validate_unit_default_weapon(&definition) {
            summary.rows_failed += 1;
            summary
                .warnings
                .push(format!("row {}: weapon validation: {err}", row.row_number));
            continue;
        }

        if let Some(profile_id) = &definition.animation_profile_id {
            if animation_profiles.get(profile_id).is_none() {
                summary.rows_failed += 1;
                summary.warnings.push(format!(
                    "row {}: unknown Animation Profile `{}`",
                    row.row_number,
                    profile_id.as_str()
                ));
                continue;
            }
        }

        if let Some(profile_id) = &definition.inventory_profile_id {
            if let Err(err) = inventory_profiles.validate_profile_reference(
                "unit",
                definition.id.as_str(),
                profile_id,
            ) {
                summary.rows_failed += 1;
                summary.warnings.push(format!(
                    "row {}: inventory profile validation: {err}",
                    row.row_number
                ));
                continue;
            }
        }

        if let Some(profile_id) = &definition.appearance_profile_id {
            let Some(profile) = appearance_profiles.get(profile_id) else {
                summary.rows_failed += 1;
                summary.warnings.push(format!(
                    "row {}: unknown Appearance Profile `{}`",
                    row.row_number,
                    profile_id.as_str()
                ));
                continue;
            };
            let Some(variant_id) = &definition.default_body_variant_id else {
                summary.rows_failed += 1;
                summary.warnings.push(format!(
                    "row {}: Appearance Profile `{}` requires Default Body Variant ID",
                    row.row_number,
                    profile_id.as_str()
                ));
                continue;
            };
            if profile.body_variant(variant_id).is_none() {
                summary.rows_failed += 1;
                summary.warnings.push(format!(
                    "row {}: unknown Default Body Variant `{}` for Appearance Profile `{}`",
                    row.row_number,
                    variant_id.as_str(),
                    profile_id.as_str()
                ));
                continue;
            }
        } else if definition.default_body_variant_id.is_some() {
            summary.rows_failed += 1;
            summary.warnings.push(format!(
                "row {}: Default Body Variant ID requires Appearance Profile ID",
                row.row_number
            ));
            continue;
        }

        let id = definition.id.clone();
        if let Some(first_row) = seen_ids.insert(id.clone(), row.row_number) {
            return Err(crate::data_import::DataImportError::DuplicateUnitId {
                id,
                first_row,
                duplicate_row: row.row_number,
            });
        }

        if row.enabled_was_blank {
            summary.warnings.push(format!(
                "row {}: Enabled blank — defaulting to true",
                row.row_number
            ));
        }

        let legacy_scale = definition.render_scale;
        let sizing_report = crate::world::finalize_unit_definition(&mut definition, legacy_scale);
        summary.sizing_reports.push(sizing_report.clone());
        for warning in sizing_report.warnings {
            summary
                .warnings
                .push(format!("row {} sizing: {warning}", row.row_number));
        }
        for error in sizing_report.errors {
            summary
                .warnings
                .push(format!("row {} sizing error: {error}", row.row_number));
        }

        definitions.push(definition);
        summary.rows_valid += 1;
    }

    if summary.rows_valid == 0 {
        return Err(crate::data_import::DataImportError::NoValidRows);
    }

    Ok((definitions, summary))
}

#[cfg(all(feature = "data-import", test))]
mod integration_tests {
    use super::*;
    use crate::world::{UnitDefinitionId, WeaponCatalog};
    use excel::UNITS_SHEET_NAME;
    use rust_xlsxwriter::Workbook;
    use std::path::{Path, PathBuf};

    fn write_workbook(path: &Path, headers: &[&str], rows: &[Vec<&str>]) {
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name(UNITS_SHEET_NAME).unwrap();
        for (col, header) in headers.iter().enumerate() {
            sheet.write_string(0, col as u16, *header).unwrap();
        }
        for (row_idx, row) in rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                sheet
                    .write_string((row_idx + 1) as u32, col as u16, *value)
                    .unwrap();
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        workbook.save(path).unwrap();
    }

    fn full_headers() -> Vec<&'static str> {
        vec![
            "Unit ID",
            "Name",
            "Faction Key",
            "Species Key",
            "Level",
            "Base HP",
            "Max HP",
            "Strength",
            "Dexterity",
            "Constitution",
            "Agility",
            "Charisma",
            "Intelligence",
            "Total Stats",
            "Power Rating",
            "Tier",
            "Default Weapon ID",
            "File Path",
            "Move Speed",
            "Collision Radius",
            "Max Slope",
            "Enabled",
        ]
    }

    fn temp_workbook(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "chasma_unit_mod_{}_{}.xlsx",
            std::process::id(),
            name
        ))
    }

    fn starter_weapons() -> WeaponCatalog {
        WeaponCatalog::default()
    }

    fn starter_factions() -> crate::world::FactionCatalog {
        crate::world::FactionCatalog::default()
    }

    fn starter_species() -> crate::world::SpeciesCatalog {
        crate::world::SpeciesCatalog::default()
    }

    fn wolf_row_prefix() -> [&'static str; 4] {
        ["U-0001", "Wolf", "wild", "wolf"]
    }

    fn wolf_row_suffix() -> [&'static str; 6] {
        [
            "26.5",
            "Elite",
            "weapon_wolf_bite",
            r"\units\wolf.glb",
            "4.5",
            "0.6",
        ]
    }

    fn wolf_row_tail() -> [&'static str; 2] {
        ["40", "Y"]
    }

    fn wolf_row() -> Vec<&'static str> {
        let mut row = vec![
            "U-0001", "Wolf", "wild", "wolf", "2", "5", "5", "4", "6", "3", "7", "2", "3", "25",
        ];
        row.extend_from_slice(&wolf_row_suffix());
        row.extend_from_slice(&wolf_row_tail());
        row
    }

    fn starter_animation_profiles() -> crate::world::AnimationProfileCatalog {
        crate::world::AnimationProfileCatalog::default()
    }

    #[test]
    fn import_end_to_end_preserves_stats() {
        let path = temp_workbook("e2e");
        let headers = full_headers();
        let row = wolf_row();
        write_workbook(&path, &headers, &[row]);
        let (definitions, summary) = import_units_from_excel(
            &path,
            &starter_factions(),
            &starter_species(),
            &starter_weapons(),
            &starter_animation_profiles(),
            &crate::world::InventoryProfileCatalog::default(),
            &crate::world::AppearanceProfileCatalog::empty(),
        )
        .unwrap();
        assert_eq!(summary.rows_valid, 1);
        let def = &definitions[0];
        assert_eq!(def.id.as_str(), "U-0001");
        assert_eq!(def.strength, 4);
        assert_eq!(def.agility, 7);
        assert_eq!(def.render_key.0.as_deref(), Some("wolf"));
        assert_eq!(def.default_weapon_id.as_str(), "weapon_wolf_bite");
        assert_eq!(def.max_hp, 5);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn disabled_rows_excluded_from_catalog() {
        let path = temp_workbook("disabled");
        let headers = full_headers();
        let rows = vec![
            wolf_row(),
            vec![
                "U-0002",
                "Deer",
                "wild",
                "deer",
                "1",
                "4",
                "4",
                "2",
                "5",
                "2",
                "8",
                "1",
                "2",
                "20",
                "12.0",
                "Common",
                "weapon_claws",
                r"\units\deer.glb",
                "5.5",
                "0.5",
                "30",
                "N",
            ],
        ];
        write_workbook(&path, &headers, &rows);
        let (definitions, summary) = import_units_from_excel(
            &path,
            &starter_factions(),
            &starter_species(),
            &starter_weapons(),
            &starter_animation_profiles(),
            &crate::world::InventoryProfileCatalog::default(),
            &crate::world::AppearanceProfileCatalog::empty(),
        )
        .unwrap();
        assert_eq!(summary.rows_valid, 1);
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].id.as_str(), "U-0001");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn deterministic_import_output() {
        let path = temp_workbook("deterministic");
        let headers = full_headers();
        let rows = vec![vec![
            "U-0003",
            "Bandit Scout",
            "bandits",
            "human",
            "3",
            "8",
            "8",
            "4",
            "7",
            "3",
            "6",
            "3",
            "4",
            "27",
            "31.6",
            "Elite",
            "weapon_fists",
            r"\units\bandit.glb",
            "3.8",
            "0.45",
            "35",
            "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let profiles = crate::world::InventoryProfileCatalog::default();
        let a = import_units_from_excel(
            &path,
            &starter_factions(),
            &starter_species(),
            &starter_weapons(),
            &starter_animation_profiles(),
            &profiles,
        &crate::world::AppearanceProfileCatalog::empty(),
        )
        .unwrap();
        let b = import_units_from_excel(
            &path,
            &starter_factions(),
            &starter_species(),
            &starter_weapons(),
            &starter_animation_profiles(),
            &profiles,
        &crate::world::AppearanceProfileCatalog::empty(),
        )
        .unwrap();
        assert_eq!(a, b);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn duplicate_unit_id_aborts_import() {
        let path = temp_workbook("duplicate");
        let headers = full_headers();
        let rows = vec![wolf_row(), {
            let mut row = wolf_row();
            row[1] = "Wolf Duplicate";
            row
        }];
        write_workbook(&path, &headers, &rows);
        let err = import_units_from_excel(
            &path,
            &starter_factions(),
            &starter_species(),
            &starter_weapons(),
            &starter_animation_profiles(),
            &crate::world::InventoryProfileCatalog::default(),
            &crate::world::AppearanceProfileCatalog::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            crate::data_import::DataImportError::DuplicateUnitId { .. }
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn lookup_by_imported_unit_id() {
        let path = temp_workbook("lookup");
        let headers = full_headers();
        let rows = vec![wolf_row()];
        write_workbook(&path, &headers, &rows);
        let (definitions, _) = import_units_from_excel(
            &path,
            &starter_factions(),
            &starter_species(),
            &starter_weapons(),
            &starter_animation_profiles(),
            &crate::world::InventoryProfileCatalog::default(),
            &crate::world::AppearanceProfileCatalog::empty(),
        )
        .unwrap();
        let catalog = crate::world::UnitCatalog::from_definitions(definitions).unwrap();
        assert!(catalog.get(&UnitDefinitionId::new("U-0001")).is_some());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn enabled_unit_missing_weapon_skips_row() {
        let path = temp_workbook("missing_weapon");
        let headers = full_headers();
        let rows = vec![vec![
            "U-0001",
            "Wolf",
            "wild",
            "wolf",
            "2",
            "5",
            "5",
            "4",
            "6",
            "3",
            "7",
            "2",
            "3",
            "25",
            "26.5",
            "Elite",
            "weapon_missing",
            r"\units\wolf.glb",
            "4.5",
            "0.6",
            "40",
            "Y",
        ]];
        write_workbook(&path, &headers, &rows);
        let err = import_units_from_excel(
            &path,
            &starter_factions(),
            &starter_species(),
            &starter_weapons(),
            &starter_animation_profiles(),
            &crate::world::InventoryProfileCatalog::default(),
            &crate::world::AppearanceProfileCatalog::empty(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            crate::data_import::DataImportError::NoValidRows
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn import_from_design() {
        let path = Path::new("Chasma Design.xlsx");
        if !path.exists() {
            return;
        }

        let weapons = starter_weapons();
        let (definitions, summary) = import_units_from_excel(
            path,
            &starter_factions(),
            &starter_species(),
            &weapons,
            &starter_animation_profiles(),
            &crate::world::InventoryProfileCatalog::default(),
            &crate::world::AppearanceProfileCatalog::empty(),
        )
        .unwrap();
        assert!(
            summary.rows_valid >= 1,
            "expected design units; valid={} failed={} warnings={:?}",
            summary.rows_valid,
            summary.rows_failed,
            summary.warnings,
        );

        let robot = definitions
            .iter()
            .find(|def| def.id.as_str() == "U-0001")
            .expect("U-0001 Robot");
        assert_eq!(robot.display_name, "Robot");
        assert_eq!(robot.faction_id.as_str(), "player");
        assert_eq!(robot.species_id.as_str(), "robot");
        assert_eq!(robot.faction_tag, "Player");
        assert!((robot.move_speed_mps - 9.0).abs() < f32::EPSILON);
        assert_eq!(robot.render_key.0.as_deref(), Some("robot"));
        assert!(
            robot.render_scale > 0.0,
            "expected positive render scale, got {}",
            robot.render_scale
        );
    }

    #[test]
    fn import_human_player_units_from_design() {
        let path = Path::new("Chasma Design.xlsx");
        if !path.exists() {
            return;
        }

        let (animation_profiles, _) =
            crate::data_import::import_animation_profiles_from_excel(path).unwrap();
        let profile_catalog =
            crate::world::AnimationProfileCatalog::from_definitions(animation_profiles).unwrap();

        let weapons = starter_weapons();
        let (definitions, summary) = import_units_from_excel(
            path,
            &starter_factions(),
            &starter_species(),
            &weapons,
            &profile_catalog,
            &crate::world::InventoryProfileCatalog::default(),
            &crate::world::AppearanceProfileCatalog::empty(),
        )
        .unwrap();
        let human_base = profile_catalog
            .get(&crate::world::AnimationProfileId::new("human_base"))
            .expect("human_base profile");
        assert_eq!(human_base.idle_clip, "Idle");
        assert_eq!(human_base.walk_clip.as_deref(), Some("Walk"));
        assert_eq!(human_base.run_clip.as_deref(), Some("Run"));
        assert_eq!(human_base.work_clip.as_deref(), Some("Mine"));
        assert_eq!(human_base.death_clip.as_deref(), Some("Death"));
        assert_eq!(human_base.hit_reaction_clip.as_deref(), Some("Hit"));
        assert_eq!(human_base.upper_body_split_bone.as_deref(), Some("spine_02"));

        for unit_id in ["U-0004", "U-0005"] {
            let def = definitions
                .iter()
                .find(|def| def.id.as_str() == unit_id)
                .expect(unit_id);
            assert_eq!(def.faction_id.as_str(), "player");
            assert_eq!(def.species_id.as_str(), "human");
            assert_eq!(def.default_weapon_id.as_str(), "weapon_fists");
            assert_eq!(
                def.inventory_profile_id.as_ref().map(|id| id.as_str()),
                Some("unit_backpack_standard"),
            );
            assert_eq!(
                def.animation_profile_id.as_ref().map(|id| id.as_str()),
                Some("human_base"),
            );
            assert!(def.work_capabilities.can_construct);
            assert!(def.work_capabilities.can_operate_workstation);
            assert!(def.work_capabilities.can_haul);
            assert!(def.enabled);
            assert!(def.render_key.0.is_some());
            weapons
                .get(&def.default_weapon_id)
                .expect("default weapon resolves");
        }

        let male = definitions
            .iter()
            .find(|def| def.id.as_str() == "U-0004")
            .expect("Human Male");
        let female = definitions
            .iter()
            .find(|def| def.id.as_str() == "U-0005")
            .expect("Human Female");
        assert_eq!(male.render_key.0.as_deref(), Some("human_male"));
        assert_eq!(female.render_key.0.as_deref(), Some("human_female"));
        assert_eq!(male.display_name, "Human Male");
        assert_eq!(female.display_name, "Human Female");
        for def in [male, female] {
            let source = def
                .asset_sizing
                .calculated_source_bounds
                .expect("asset sizing baked");
            assert!(
                source.height_meters > 1.5 && source.height_meters < 2.1,
                "{} source height {:.3}m",
                def.display_name,
                source.height_meters,
            );
            assert!(def.render_scale > 0.5 && def.render_scale < 1.5);
        }
    }
}
