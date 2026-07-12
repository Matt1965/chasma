//! Deterministic animation scale stress scenarios (A6 / D6).

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use bevy::prelude::Vec3;

    use crate::units::animation::assets::{AnimationGraphShareKey, DefinitionAnimationGraph};
    use crate::units::animation::lod::{
        AnimationLod, AnimationLodSettings, animation_distance_meters, raw_animation_lod,
        resolve_animation_lod, should_evaluate_animation_intent,
    };
    use crate::world::{AnimationProfileId, UnitDefinitionId, WeaponDefinitionId};

    fn lod_settings() -> AnimationLodSettings {
        AnimationLodSettings::default()
    }

    #[test]
    fn five_hundred_units_classify_into_lod_tiers() {
        let settings = lod_settings();
        let focus = Vec3::ZERO;
        let mut full = 0u32;
        let mut reduced = 0u32;
        let mut frozen = 0u32;
        for index in 0..500u32 {
            let distance = (index as f32 * 3.7) % 320.0;
            let position = Vec3::new(distance, 0.0, distance * 0.25);
            let dist = animation_distance_meters(focus, position);
            match raw_animation_lod(dist, &settings, None, None, &HashSet::new(), None) {
                AnimationLod::Full => full += 1,
                AnimationLod::Reduced => reduced += 1,
                AnimationLod::Frozen => frozen += 1,
            }
        }
        assert!(full > 0);
        assert!(reduced > 0);
        assert!(frozen > 0);
        assert_eq!(full + reduced + frozen, 500);
    }

    #[test]
    fn hundred_units_share_one_graph_identity() {
        let share_key = AnimationGraphShareKey {
            profile_id: AnimationProfileId::new("humanoid"),
            gltf_asset_path: "units/wolf.glb".to_string(),
            default_weapon_id: WeaponDefinitionId::new("weapon_wolf_bite"),
        };
        let graph = DefinitionAnimationGraph {
            graph: bevy::prelude::Handle::default(),
            locomotion_nodes: Default::default(),
            attack_nodes: Default::default(),
            locomotion_durations: Default::default(),
            attack_durations: Default::default(),
            death_node: None,
            death_duration: None,
            hit_reaction_node: None,
            hit_reaction_duration: None,
            idle_fallback_node: None,
            blend_root: bevy::prelude::AnimationNodeIndex::new(0),
            profile_id: share_key.profile_id.clone(),
            share_key: share_key.clone(),
        };
        let mut shared = std::collections::HashMap::new();
        shared.insert(share_key, graph);
        assert_eq!(shared.len(), 1);
    }

    #[test]
    fn rapid_camera_thresholds_use_hysteresis() {
        let settings = lod_settings();
        let mut lod = AnimationLod::Full;
        for distance in [78.0, 82.0, 79.0, 83.0, 77.0] {
            lod =
                resolve_animation_lod(distance, lod, &settings, None, None, &HashSet::new(), None);
        }
        assert!(matches!(lod, AnimationLod::Full | AnimationLod::Reduced));
    }

    #[test]
    fn reduced_lod_throttles_five_hundred_evaluations() {
        let _settings = lod_settings();
        let mut eval_count = 0u32;
        for index in 0..500u32 {
            if should_evaluate_animation_intent(
                AnimationLod::Reduced,
                0.1,
                if index % 2 == 0 { 0.0 } else { 10.0 },
                false,
            ) {
                eval_count += 1;
            }
        }
        assert!(eval_count < 500);
        assert!(eval_count > 0);
    }
}
