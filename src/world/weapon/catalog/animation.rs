use bevy::prelude::*;

/// How attack clips are scaled against authoritative weapon timing (A2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum AttackPlaybackPolicy {
    /// Scale clip duration to fit windup + recovery; simulation timing wins (A2).
    #[default]
    ScaleToCycle,
}

/// Weapon-owned attack animation parameters (A2).
///
/// Locomotion clips remain on [`crate::world::AnimationProfile`]; attack clips are
/// keyed by [`super::definition::WeaponDefinition::animation_key`].
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct WeaponAttackAnimation {
    pub playback_policy: AttackPlaybackPolicy,
    /// Normalized clip time (0–1) where the strike visually lands (A2).
    pub normalized_strike_time: f32,
    pub blend_in_ms: u32,
    pub blend_out_ms: u32,
    /// Reserved for future variant selection (A2+).
    pub variant: Option<String>,
}

impl Default for WeaponAttackAnimation {
    fn default() -> Self {
        Self {
            playback_policy: AttackPlaybackPolicy::ScaleToCycle,
            normalized_strike_time: 0.42,
            blend_in_ms: 150,
            blend_out_ms: 150,
            variant: None,
        }
    }
}

impl WeaponAttackAnimation {
    pub fn normalized_strike_time_clamped(&self) -> f32 {
        self.normalized_strike_time.clamp(0.0, 1.0)
    }
}

impl AttackPlaybackPolicy {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "scaletocycle" | "scale_to_cycle" | "scale to cycle" => Ok(Self::ScaleToCycle),
            other => Err(format!("unknown Attack Playback Policy `{other}`")),
        }
    }
}
