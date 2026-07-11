use super::zone::DoodadExclusionZone;
use crate::world::doodad::generation::DoodadSpawnCandidate;
use crate::world::{ChunkLayout, WorldPosition};

/// Output of [`filter_candidates_by_exclusion_zones`].
#[derive(Debug, Clone, PartialEq)]
pub struct ExclusionFilterResult {
    pub retained: Vec<DoodadSpawnCandidate>,
    pub excluded_count: u32,
}

/// World-space distance between two authoritative positions (ADR-001).
pub fn world_position_distance(a: WorldPosition, b: WorldPosition, layout: ChunkLayout) -> f32 {
    a.to_global(layout).distance(b.to_global(layout))
}

/// Returns `true` when `position` lies inside any zone (inclusive radius boundary).
pub fn position_excluded_by_zones(
    position: WorldPosition,
    zones: &[DoodadExclusionZone],
    layout: ChunkLayout,
) -> bool {
    zones
        .iter()
        .any(|zone| world_position_distance(position, zone.center, layout) <= zone.radius_meters)
}

/// Pure, deterministic procedural candidate filter (ADR-020).
///
/// Preserves relative order of retained candidates. Side-effect free.
pub fn filter_candidates_by_exclusion_zones(
    candidates: &[DoodadSpawnCandidate],
    zones: &[DoodadExclusionZone],
    layout: ChunkLayout,
) -> ExclusionFilterResult {
    if zones.is_empty() {
        return ExclusionFilterResult {
            retained: candidates.to_vec(),
            excluded_count: 0,
        };
    }

    let mut retained = Vec::with_capacity(candidates.len());
    let mut excluded_count = 0u32;

    for candidate in candidates {
        if position_excluded_by_zones(candidate.position, zones, layout) {
            excluded_count += 1;
        } else {
            retained.push(candidate.clone());
        }
    }

    ExclusionFilterResult {
        retained,
        excluded_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, DoodadDefinitionId, DoodadSource, LocalPosition};
    use bevy::prelude::{Quat, Vec3};

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn candidate_at(x: f32, z: f32) -> DoodadSpawnCandidate {
        DoodadSpawnCandidate {
            definition_id: DoodadDefinitionId::new("tree_oak"),
            source: DoodadSource::Procedural { seed: 1 },
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(x, 0.0, z)),
            ),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    fn zone_at(x: f32, z: f32, radius: f32) -> DoodadExclusionZone {
        DoodadExclusionZone::new(
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(x, 0.0, z)),
            ),
            radius,
        )
    }

    #[test]
    fn candidate_inside_exclusion_zone_removed() {
        let layout = layout();
        let zones = vec![zone_at(50.0, 50.0, 20.0)];
        let candidates = vec![candidate_at(55.0, 55.0)];

        let result = filter_candidates_by_exclusion_zones(&candidates, &zones, layout);

        assert!(result.retained.is_empty());
        assert_eq!(result.excluded_count, 1);
    }

    #[test]
    fn candidate_outside_exclusion_zone_survives() {
        let layout = layout();
        let zones = vec![zone_at(50.0, 50.0, 10.0)];
        let candidates = vec![candidate_at(200.0, 200.0)];

        let result = filter_candidates_by_exclusion_zones(&candidates, &zones, layout);

        assert_eq!(result.retained.len(), 1);
        assert_eq!(result.excluded_count, 0);
    }

    #[test]
    fn multiple_exclusion_zones() {
        let layout = layout();
        let zones = vec![zone_at(10.0, 10.0, 5.0), zone_at(200.0, 200.0, 15.0)];
        let candidates = vec![
            candidate_at(12.0, 12.0),
            candidate_at(100.0, 100.0),
            candidate_at(205.0, 205.0),
        ];

        let result = filter_candidates_by_exclusion_zones(&candidates, &zones, layout);

        assert_eq!(result.retained.len(), 1);
        assert_eq!(result.retained[0].position.local.0.x, 100.0);
        assert_eq!(result.excluded_count, 2);
    }

    #[test]
    fn boundary_exact_radius_is_excluded() {
        let layout = layout();
        let zones = vec![zone_at(0.0, 0.0, 10.0)];
        let candidates = vec![candidate_at(10.0, 0.0)];

        let dist = world_position_distance(candidates[0].position, zones[0].center, layout);
        assert!((dist - 10.0).abs() < 1e-4);

        let result = filter_candidates_by_exclusion_zones(&candidates, &zones, layout);
        assert!(result.retained.is_empty());
        assert_eq!(result.excluded_count, 1);
    }

    #[test]
    fn filter_is_deterministic() {
        let layout = layout();
        let zones = vec![zone_at(64.0, 64.0, 32.0)];
        let candidates = vec![
            candidate_at(10.0, 10.0),
            candidate_at(64.0, 64.0),
            candidate_at(200.0, 200.0),
        ];

        let a = filter_candidates_by_exclusion_zones(&candidates, &zones, layout);
        let b = filter_candidates_by_exclusion_zones(&candidates, &zones, layout);
        assert_eq!(a, b);
    }
}
