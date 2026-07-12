use crate::world::{AnimationProfile, AnimationProfileId};

pub const REQUIRED_COLUMNS: &[&str] = &["Profile ID", "Idle Animation"];

pub const OPTIONAL_COLUMNS: &[&str] = &[
    "Walk Animation",
    "Run Animation",
    "Locomotion Reference Speed",
    "Enabled",
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
}

impl AnimationProfileImportRow {
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
    }
}
