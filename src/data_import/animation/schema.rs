use crate::world::{AnimationProfile, AnimationProfileId};

pub const REQUIRED_COLUMNS: &[&str] = &["Profile ID", "Idle Animation"];

pub const OPTIONAL_COLUMNS: &[&str] = &[
    "Walk Animation",
    "Run Animation",
    "Locomotion Reference Speed",
    "Enabled",
    "Death Animation",
    "Hit Reaction Animation",
    "Upper Body Split Bone",
    "Turn Left Animation",
    "Turn Right Animation",
    "Turn Left Duration",
    "Turn Right Duration",
];

pub const DEFAULT_LOCOMOTION_REFERENCE_SPEED_MPS: f32 = 4.0;

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationProfileImportRow {
    pub row_number: usize,
    pub profile_id: String,
    pub idle_animation: String,
    pub walk_animation: String,
    pub run_animation: String,
    pub locomotion_reference_speed_mps: f32,
    pub enabled: bool,
    pub enabled_was_blank: bool,
    pub has_walk_column: bool,
    pub has_run_column: bool,
    pub has_reference_speed_column: bool,
    pub death_animation: String,
    pub hit_reaction_animation: String,
    pub upper_body_split_bone: String,
    pub turn_left_animation: String,
    pub turn_right_animation: String,
    pub turn_left_duration_seconds: Option<f32>,
    pub turn_right_duration_seconds: Option<f32>,
    pub has_death_column: bool,
    pub has_hit_reaction_column: bool,
    pub has_upper_body_split_bone_column: bool,
    pub has_turn_left_column: bool,
    pub has_turn_right_column: bool,
    pub has_turn_left_duration_column: bool,
    pub has_turn_right_duration_column: bool,
}

impl AnimationProfileImportRow {
    fn optional_clip(column_present: bool, value: &str) -> Option<String> {
        if column_present && !value.trim().is_empty() {
            Some(value.trim().to_string())
        } else {
            None
        }
    }

    pub fn to_definition(&self) -> AnimationProfile {
        let walk = if self.has_walk_column && !self.walk_animation.trim().is_empty() {
            Some(self.walk_animation.trim().to_string())
        } else {
            None
        };
        let run = if self.has_run_column && !self.run_animation.trim().is_empty() {
            Some(self.run_animation.trim().to_string())
        } else {
            None
        };

        AnimationProfile::new(
            AnimationProfileId::new(self.profile_id.trim()),
            self.idle_animation.trim(),
            walk,
            run,
            self.locomotion_reference_speed_mps,
            self.enabled,
        )
        .with_presentation_clips(
            Self::optional_clip(self.has_death_column, &self.death_animation),
            Self::optional_clip(self.has_hit_reaction_column, &self.hit_reaction_animation),
        )
        .with_layering(Self::optional_clip(
            self.has_upper_body_split_bone_column,
            &self.upper_body_split_bone,
        ))
        .with_turn_clips(
            Self::optional_clip(self.has_turn_left_column, &self.turn_left_animation),
            Self::optional_clip(self.has_turn_right_column, &self.turn_right_animation),
            self.turn_left_duration_seconds,
            self.turn_right_duration_seconds,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_row() -> AnimationProfileImportRow {
        AnimationProfileImportRow {
            row_number: 2,
            profile_id: "humanoid".to_string(),
            idle_animation: "Idle".to_string(),
            walk_animation: "Walk".to_string(),
            run_animation: "Run".to_string(),
            locomotion_reference_speed_mps: 4.0,
            enabled: true,
            enabled_was_blank: false,
            has_walk_column: true,
            has_run_column: true,
            has_reference_speed_column: true,
            death_animation: String::new(),
            hit_reaction_animation: String::new(),
            upper_body_split_bone: String::new(),
            turn_left_animation: String::new(),
            turn_right_animation: String::new(),
            turn_left_duration_seconds: None,
            turn_right_duration_seconds: None,
            has_death_column: false,
            has_hit_reaction_column: false,
            has_upper_body_split_bone_column: false,
            has_turn_left_column: false,
            has_turn_right_column: false,
            has_turn_left_duration_column: false,
            has_turn_right_duration_column: false,
        }
    }

    #[test]
    fn legacy_row_without_new_columns_omits_presentation_fields() {
        let profile = base_row().to_definition();
        assert_eq!(profile.idle_clip, "Idle");
        assert_eq!(profile.walk_clip.as_deref(), Some("Walk"));
        assert_eq!(profile.death_clip, None);
        assert_eq!(profile.hit_reaction_clip, None);
        assert_eq!(profile.upper_body_split_bone, None);
        assert_eq!(profile.turn_left_clip, None);
        assert_eq!(profile.turn_right_clip, None);
        assert_eq!(profile.turn_left_duration_seconds, None);
        assert_eq!(profile.turn_right_duration_seconds, None);
    }

    #[test]
    fn populated_optional_columns_map_to_animation_profile() {
        let mut row = base_row();
        row.has_death_column = true;
        row.death_animation = "Death".to_string();
        row.has_hit_reaction_column = true;
        row.hit_reaction_animation = "GetHit1".to_string();
        row.has_turn_left_column = true;
        row.turn_left_animation = "CrawlLeft".to_string();
        row.has_turn_right_column = true;
        row.turn_right_animation = "CrawlRight".to_string();
        row.has_turn_left_duration_column = true;
        row.turn_left_duration_seconds = Some(1.0);
        row.has_turn_right_duration_column = true;
        row.turn_right_duration_seconds = Some(1.0);

        let profile = row.to_definition();
        assert_eq!(profile.death_clip.as_deref(), Some("Death"));
        assert_eq!(profile.hit_reaction_clip.as_deref(), Some("GetHit1"));
        assert_eq!(profile.turn_left_clip.as_deref(), Some("CrawlLeft"));
        assert_eq!(profile.turn_right_clip.as_deref(), Some("CrawlRight"));
        assert_eq!(profile.turn_left_duration_seconds, Some(1.0));
        assert_eq!(profile.turn_right_duration_seconds, Some(1.0));
    }

    #[test]
    fn blank_optional_cells_become_none_even_when_columns_exist() {
        let mut row = base_row();
        row.has_death_column = true;
        row.has_hit_reaction_column = true;
        row.has_upper_body_split_bone_column = true;
        row.has_turn_left_column = true;
        row.has_turn_right_column = true;
        row.has_turn_left_duration_column = true;
        row.has_turn_right_duration_column = true;

        let profile = row.to_definition();
        assert_eq!(profile.death_clip, None);
        assert_eq!(profile.hit_reaction_clip, None);
        assert_eq!(profile.upper_body_split_bone, None);
        assert_eq!(profile.turn_left_clip, None);
        assert_eq!(profile.turn_right_clip, None);
        assert_eq!(profile.turn_left_duration_seconds, None);
        assert_eq!(profile.turn_right_duration_seconds, None);
    }

    #[test]
    fn idle_walk_run_mapping_unchanged() {
        let profile = base_row().to_definition();
        assert_eq!(profile.locomotion_reference_speed_mps, 4.0);
        assert!(profile.enabled);
        let (name, key) = profile
            .resolve_clip_name(crate::world::AnimationClipKey::Run)
            .unwrap();
        assert_eq!(key, crate::world::AnimationClipKey::Run);
        assert_eq!(name, "Run");
    }
}
