use bevy::prelude::*;

/// Fingerprint of the last morph weights applied to a unit render instance.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct UnitAppearanceMorphFingerprint {
    pub profile_id: String,
    pub body_variant_id: String,
    pub height_scale: f32,
    pub morph_digest: u64,
}

impl UnitAppearanceMorphFingerprint {
    pub fn from_appearance(
        profile_id: &str,
        body_variant_id: &str,
        height_scale: f32,
        morph_digest: u64,
    ) -> Self {
        Self {
            profile_id: profile_id.to_string(),
            body_variant_id: body_variant_id.to_string(),
            height_scale,
            morph_digest,
        }
    }
}
