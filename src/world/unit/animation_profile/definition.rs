use bevy::prelude::*;

use super::id::AnimationProfileId;

/// Logical locomotion clip keys resolved through [`AnimationProfile`] (A1/A5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum AnimationClipKey {
    #[default]
    Idle,
    Walk,
    Run,
    TurnLeft,
    TurnRight,
}

impl AnimationClipKey {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Walk => "Walk",
            Self::Run => "Run",
            Self::TurnLeft => "TurnLeft",
            Self::TurnRight => "TurnRight",
        }
    }

    pub fn is_turn(self) -> bool {
        matches!(self, Self::TurnLeft | Self::TurnRight)
    }
}

/// Authoritative animation profile: clip name mapping for one locomotion set (A1).
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct AnimationProfile {
    pub id: AnimationProfileId,
    pub idle_clip: String,
    pub walk_clip: Option<String>,
    pub run_clip: Option<String>,
    /// Clip-authored locomotion speed used for playback scaling (m/s).
    pub locomotion_reference_speed_mps: f32,
    pub death_clip: Option<String>,
    pub hit_reaction_clip: Option<String>,
    pub death_blend_ms: u32,
    pub hit_reaction_blend_ms: u32,
    /// First upper-body bone name or path suffix for masked layering (A4).
    /// When absent, playback falls back to full-body exclusive clips.
    pub upper_body_split_bone: Option<String>,
    pub turn_left_clip: Option<String>,
    pub turn_right_clip: Option<String>,
    pub turn_left_duration_seconds: Option<f32>,
    pub turn_right_duration_seconds: Option<f32>,
    pub enabled: bool,
}

impl AnimationProfile {
    pub fn new(
        id: AnimationProfileId,
        idle_clip: impl Into<String>,
        walk_clip: Option<String>,
        run_clip: Option<String>,
        locomotion_reference_speed_mps: f32,
        enabled: bool,
    ) -> Self {
        Self {
            id,
            idle_clip: idle_clip.into(),
            walk_clip,
            run_clip,
            locomotion_reference_speed_mps,
            death_clip: None,
            hit_reaction_clip: None,
            death_blend_ms: 200,
            hit_reaction_blend_ms: 80,
            upper_body_split_bone: None,
            turn_left_clip: None,
            turn_right_clip: None,
            turn_left_duration_seconds: None,
            turn_right_duration_seconds: None,
            enabled,
        }
    }

    pub fn with_presentation_clips(
        mut self,
        death_clip: Option<String>,
        hit_reaction_clip: Option<String>,
    ) -> Self {
        self.death_clip = death_clip;
        self.hit_reaction_clip = hit_reaction_clip;
        self
    }

    pub fn with_layering(mut self, upper_body_split_bone: Option<String>) -> Self {
        self.upper_body_split_bone = upper_body_split_bone;
        self
    }

    pub fn with_turn_clips(
        mut self,
        turn_left_clip: Option<String>,
        turn_right_clip: Option<String>,
        turn_left_duration_seconds: Option<f32>,
        turn_right_duration_seconds: Option<f32>,
    ) -> Self {
        self.turn_left_clip = turn_left_clip;
        self.turn_right_clip = turn_right_clip;
        self.turn_left_duration_seconds = turn_left_duration_seconds;
        self.turn_right_duration_seconds = turn_right_duration_seconds;
        self
    }

    pub fn turn_duration_seconds(&self, clip: AnimationClipKey) -> Option<f32> {
        match clip {
            AnimationClipKey::TurnLeft => self.turn_left_duration_seconds,
            AnimationClipKey::TurnRight => self.turn_right_duration_seconds,
            _ => None,
        }
    }

    pub fn layering_split_bone(&self) -> Option<&str> {
        self.upper_body_split_bone
            .as_deref()
            .filter(|bone| !bone.is_empty())
    }

    pub fn resolve_death_clip_name(&self) -> Option<&str> {
        self.death_clip.as_deref().filter(|name| !name.is_empty())
    }

    pub fn resolve_hit_reaction_clip_name(&self) -> Option<&str> {
        self.hit_reaction_clip
            .as_deref()
            .filter(|name| !name.is_empty())
    }

    /// Resolve a desired clip to a concrete glTF clip name with Run → Walk → Idle fallback.
    pub fn resolve_clip_name(&self, desired: AnimationClipKey) -> Option<(&str, AnimationClipKey)> {
        let chain = match desired {
            AnimationClipKey::Run => [
                AnimationClipKey::Run,
                AnimationClipKey::Walk,
                AnimationClipKey::Idle,
            ],
            AnimationClipKey::Walk => [
                AnimationClipKey::Walk,
                AnimationClipKey::Idle,
                AnimationClipKey::Idle,
            ],
            AnimationClipKey::Idle => [
                AnimationClipKey::Idle,
                AnimationClipKey::Idle,
                AnimationClipKey::Idle,
            ],
            AnimationClipKey::TurnLeft | AnimationClipKey::TurnRight => [desired, desired, desired],
        };
        for key in chain {
            if let Some(name) = self.clip_name_for_key(key) {
                return Some((name, key));
            }
        }
        None
    }

    fn clip_name_for_key(&self, key: AnimationClipKey) -> Option<&str> {
        match key {
            AnimationClipKey::Idle => {
                if self.idle_clip.is_empty() {
                    None
                } else {
                    Some(self.idle_clip.as_str())
                }
            }
            AnimationClipKey::Walk => self.walk_clip.as_deref().filter(|name| !name.is_empty()),
            AnimationClipKey::Run => self.run_clip.as_deref().filter(|name| !name.is_empty()),
            AnimationClipKey::TurnLeft => self
                .turn_left_clip
                .as_deref()
                .filter(|name| !name.is_empty()),
            AnimationClipKey::TurnRight => self
                .turn_right_clip
                .as_deref()
                .filter(|name| !name.is_empty()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> AnimationProfile {
        AnimationProfile::new(
            AnimationProfileId::new("humanoid"),
            "Idle",
            Some("Walk".to_string()),
            Some("Run".to_string()),
            4.0,
            true,
        )
    }

    #[test]
    fn run_falls_back_to_walk_then_idle() {
        let profile = AnimationProfile::new(
            AnimationProfileId::new("minimal"),
            "Idle",
            Some("Walk".to_string()),
            None,
            4.0,
            true,
        );
        let (name, key) = profile.resolve_clip_name(AnimationClipKey::Run).unwrap();
        assert_eq!(key, AnimationClipKey::Walk);
        assert_eq!(name, "Walk");
    }

    #[test]
    fn walk_falls_back_to_idle() {
        let profile = AnimationProfile::new(
            AnimationProfileId::new("idle_only"),
            "Idle",
            None,
            None,
            4.0,
            true,
        );
        let (name, key) = profile.resolve_clip_name(AnimationClipKey::Walk).unwrap();
        assert_eq!(key, AnimationClipKey::Idle);
        assert_eq!(name, "Idle");
    }

    #[test]
    fn run_resolves_directly_when_present() {
        let profile = sample_profile();
        let (name, key) = profile.resolve_clip_name(AnimationClipKey::Run).unwrap();
        assert_eq!(key, AnimationClipKey::Run);
        assert_eq!(name, "Run");
    }
}
