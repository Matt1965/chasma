/// Starter animation profiles for tests and dev fallback when the workbook sheet is absent.
#[cfg(any(test, feature = "dev"))]
mod fixtures {
    use super::super::definition::AnimationProfile;
    use super::super::id::AnimationProfileId;

    pub fn starter_definitions() -> Vec<AnimationProfile> {
        vec![
            AnimationProfile::new(
                AnimationProfileId::new("humanoid"),
                "Idle",
                Some("Walk".to_string()),
                Some("Run".to_string()),
                4.0,
                true,
            )
            .with_presentation_clips(Some("Death".to_string()), Some("Hit".to_string()))
            .with_layering(Some("Spine".to_string())),
            AnimationProfile::new(
                AnimationProfileId::new("quadruped"),
                "Idle",
                Some("Walk".to_string()),
                Some("Run".to_string()),
                4.5,
                true,
            )
            .with_presentation_clips(Some("Death".to_string()), Some("Hit".to_string()))
            .with_layering(Some("Spine".to_string())),
        ]
    }
}

#[cfg(any(test, feature = "dev"))]
pub use fixtures::starter_definitions;

#[cfg(not(any(test, feature = "dev")))]
pub fn starter_definitions() -> Vec<super::definition::AnimationProfile> {
    Vec::new()
}
