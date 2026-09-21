#[cfg(test)]
mod tests {
    use bevy::prelude::{Quat, Vec3};

    use crate::data_import::equipment_visual::schema::EquipmentVisualImportRow;
    use crate::world::equipment::DEFAULT_EQUIPMENT_FIT_SCALE;
    use crate::world::{EquipmentPresentationMode, ItemDefinitionId};

    fn base_row() -> EquipmentVisualImportRow {
        EquipmentVisualImportRow {
            row_number: 2,
            item_id: "ranger_body".to_string(),
            unit_render_key: "human_male".to_string(),
            equipped_render_key: "equipment/human_male/ranger_body".to_string(),
            presentation_mode: "SkinnedOverlay".to_string(),
            socket: None,
            local_translation: None,
            local_rotation: None,
            local_scale: None,
            stowed_socket: None,
            stowed_local_translation: None,
            stowed_local_rotation: None,
            stowed_local_scale: None,
            fit_scale: None,
            fit_offset: None,
            consumed_morph_params: Some("build,fat,muscle".to_string()),
        }
    }

    #[test]
    fn default_fit_values_preserve_legacy_mapping() {
        let mapping = base_row().to_mapping().unwrap();
        assert_eq!(mapping.fit_scale, DEFAULT_EQUIPMENT_FIT_SCALE);
        assert_eq!(mapping.fit_offset, Vec3::ZERO);
        assert_eq!(mapping.mode, EquipmentPresentationMode::SkinnedOverlay);
    }

    #[test]
    fn authored_fit_metadata_parses() {
        let mut row = base_row();
        row.fit_scale = Some("1.04".to_string());
        row.fit_offset = Some("0,0.005,0".to_string());
        let mapping = row.to_mapping().unwrap();
        assert!((mapping.fit_scale - 1.04).abs() < 1e-5);
        assert_eq!(mapping.fit_offset, Vec3::new(0.0, 0.005, 0.0));
    }

    #[test]
    fn invalid_fit_scale_fails_import() {
        let mut row = base_row();
        row.fit_scale = Some("0".to_string());
        assert!(row.to_mapping().is_err());
    }

    #[test]
    fn male_and_female_fit_mappings_remain_independent() {
        let mut male = base_row();
        male.fit_scale = Some("1.04".to_string());
        let mut female = base_row();
        female.unit_render_key = "human_female".to_string();
        female.equipped_render_key = "equipment/human_female/ranger_body".to_string();
        female.fit_scale = Some("1.035".to_string());
        let male_mapping = male.to_mapping().unwrap();
        let female_mapping = female.to_mapping().unwrap();
        assert_eq!(male_mapping.item_id, ItemDefinitionId::new("ranger_body"));
        assert_ne!(male_mapping.fit_scale, female_mapping.fit_scale);
        assert_ne!(
            male_mapping.equipped_render_key,
            female_mapping.equipped_render_key
        );
    }

    #[test]
    fn rigid_row_can_carry_fit_without_consumed_morphs() {
        let mut row = base_row();
        row.presentation_mode = "RigidAttachment".to_string();
        row.socket = Some("RightHand".to_string());
        row.consumed_morph_params = None;
        row.fit_scale = Some("1.02".to_string());
        let mapping = row.to_mapping().unwrap();
        assert_eq!(mapping.mode, EquipmentPresentationMode::RigidAttachment);
        assert!(mapping.consumed_morph_params.is_empty());
        assert!((mapping.fit_scale - 1.02).abs() < 1e-5);
        assert_eq!(mapping.local_rotation, Quat::IDENTITY);
    }
}
