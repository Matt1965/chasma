#[cfg(test)]
mod workbook_tests {
    use bevy::prelude::{Quat, Vec3};

    use crate::data_import::equipment_visual::import_equipment_visuals_from_excel;
    use crate::data_import::import_appearance_profiles_from_excel;
    use crate::data_import::import_item_catalog_from_excel;
    use crate::data_import::paths::dev_design_workbook_path;
    use crate::world::{AppearanceParamId, EquipmentPresentationMode, ItemDefinitionId};

    #[test]
    fn design_workbook_imports_equipment_visuals() {
        let path = dev_design_workbook_path();
        assert!(path.exists(), "workbook missing at {}", path.display());
        let (_, items, _) = import_item_catalog_from_excel(&path).unwrap();
        let (appearance_profiles, _) = import_appearance_profiles_from_excel(&path).unwrap();
        let (visuals, summary) =
            import_equipment_visuals_from_excel(&path, &items, &appearance_profiles).unwrap();
        assert_eq!(summary.rows_failed, 0, "warnings={:?}", summary.warnings);
        assert!(summary.rows_valid >= 30);
        let male = visuals
            .resolve(&ItemDefinitionId::new("ranger_body"), "human_male")
            .expect("male ranger body visual");
        assert_eq!(male.mode, EquipmentPresentationMode::SkinnedOverlay);
        assert_eq!(
            male.equipped_render_key.0.as_deref(),
            Some("equipment/human_male/ranger_body")
        );
        let female = visuals
            .resolve(&ItemDefinitionId::new("ranger_body"), "human_female")
            .expect("female ranger body visual");
        assert_eq!(
            female.equipped_render_key.0.as_deref(),
            Some("equipment/human_female/ranger_body")
        );
        let sword = visuals
            .resolve(&ItemDefinitionId::new("iron_sword"), "human_male")
            .expect("iron sword rigid visual");
        assert_eq!(sword.mode, EquipmentPresentationMode::RigidAttachment);
        assert!(sword.consumed_morph_params.is_empty());
        assert!(items.get(&ItemDefinitionId::new("scrap_sword")).is_none());
        assert_eq!(
            male.consumed_morph_params,
            vec![
                AppearanceParamId::new("build"),
                AppearanceParamId::new("fat"),
                AppearanceParamId::new("muscle"),
                AppearanceParamId::new("shoulders"),
                AppearanceParamId::new("torso"),
                AppearanceParamId::new("hips"),
            ]
        );
        let arms = visuals
            .resolve(&ItemDefinitionId::new("ranger_arms"), "human_male")
            .expect("ranger arms visual");
        assert_eq!(
            arms.consumed_morph_params,
            vec![
                AppearanceParamId::new("build"),
                AppearanceParamId::new("fat"),
                AppearanceParamId::new("muscle"),
                AppearanceParamId::new("arms"),
            ]
        );
        let legs = visuals
            .resolve(&ItemDefinitionId::new("peasant_legs"), "human_female")
            .expect("peasant legs visual");
        assert_eq!(
            legs.consumed_morph_params,
            vec![
                AppearanceParamId::new("build"),
                AppearanceParamId::new("fat"),
                AppearanceParamId::new("muscle"),
                AppearanceParamId::new("legs"),
                AppearanceParamId::new("hips"),
            ]
        );
        let hood = visuals
            .resolve(&ItemDefinitionId::new("ranger_hood"), "human_male")
            .expect("ranger hood visual");
        assert_eq!(
            hood.consumed_morph_params,
            vec![AppearanceParamId::new("head_size")]
        );
    }

    #[test]
    fn design_workbook_imports_equipment_visual_local_transforms() {
        let path = dev_design_workbook_path();
        let (_, items, _) = import_item_catalog_from_excel(&path).unwrap();
        let (appearance_profiles, _) = import_appearance_profiles_from_excel(&path).unwrap();
        let (visuals, summary) =
            import_equipment_visuals_from_excel(&path, &items, &appearance_profiles).unwrap();
        assert_eq!(summary.rows_failed, 0, "warnings={:?}", summary.warnings);

        let backpack = visuals
            .resolve(&ItemDefinitionId::new("leather_backpack"), "human_male")
            .expect("leather backpack visual");
        assert_eq!(backpack.local_translation, Vec3::new(0.0, 0.10, -0.16));
        assert_eq!(backpack.local_rotation, Quat::from_xyzw(0.0, 1.0, 0.0, 0.0));

        let sword = visuals
            .resolve(&ItemDefinitionId::new("iron_sword"), "human_male")
            .expect("iron sword visual");
        assert_eq!(sword.local_translation, Vec3::new(0.0, 0.183, 0.02));
        assert_eq!(sword.local_rotation, Quat::from_xyzw(0.0, -0.5, -0.5, 0.5));

        let female = visuals
            .resolve(&ItemDefinitionId::new("iron_sword"), "human_female")
            .expect("female iron sword visual");
        assert_eq!(female.local_translation, sword.local_translation);
        assert_ne!(
            visuals
                .resolve(&ItemDefinitionId::new("ranger_body"), "human_male")
                .unwrap()
                .equipped_render_key,
            visuals
                .resolve(&ItemDefinitionId::new("ranger_body"), "human_female")
                .unwrap()
                .equipped_render_key,
        );
    }
}
