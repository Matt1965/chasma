//! Bounded placement search with hard validity and soft preference (SA9).

use bevy::prelude::*;

use crate::world::building::catalog::{BuildingCatalog, BuildingDefinitionId};
use crate::world::{
    rotation_from_quadrants, validate_building_placement, xz_distance, BuildingOwnership,
    BuildingPlacementConfig, BuildingPlacementContext, DoodadCatalog, FootprintCatalog,
    UnitCatalog, WorldData, WorldPosition,
};

use super::plan::ConstructionPlacementCandidate;
use super::report::RejectedSiteDiagnostic;

#[derive(Debug, Clone, Copy)]
pub struct PlacementSearchBudget {
    pub search_radius_meters: f32,
    pub step_meters: f32,
    pub max_candidates: u32,
}

impl Default for PlacementSearchBudget {
    fn default() -> Self {
        Self {
            search_radius_meters: 48.0,
            step_meters: 8.0,
            max_candidates: 24,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlacementSearchResult {
    pub selected: Option<ConstructionPlacementCandidate>,
    pub rejected: Vec<RejectedSiteDiagnostic>,
    pub diagnostics: Vec<String>,
}

/// Search for a valid construction site near the settlement anchor.
///
/// Hard validity uses existing placement validation. Soft preference ranks valid sites only.
pub fn search_placement_candidates(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    doodad_catalog: &DoodadCatalog,
    unit_catalog: &UnitCatalog,
    definition_id: &BuildingDefinitionId,
    ownership: BuildingOwnership,
    anchor: WorldPosition,
    budget: PlacementSearchBudget,
) -> PlacementSearchResult {
    let mut rejected = Vec::new();
    let mut diagnostics = Vec::new();
    let mut valid: Vec<ConstructionPlacementCandidate> = Vec::new();

    let layout = world.layout();
    let anchor_global = anchor.to_global(layout);
    let ctx = BuildingPlacementContext {
        world,
        building_catalog,
        footprint_catalog,
        doodad_catalog,
        unit_catalog,
        config: BuildingPlacementConfig::default(),
        player_authorized: true,
    };

    let step = budget.step_meters.max(2.0);
    let radius = budget.search_radius_meters.max(step);
    let max = budget.max_candidates.max(1) as usize;

    // Deterministic spiral-ish grid from near-anchor outward (ring by ring).
    let mut offsets: Vec<(f32, f32)> = Vec::new();
    let rings = ((radius / step).ceil() as i32).max(1);
    for ring in 0..=rings {
        let r = ring as f32 * step;
        if ring == 0 {
            offsets.push((step, 0.0)); // avoid overlapping the anchor building
            continue;
        }
        let count = (ring * 4).max(4);
        for i in 0..count {
            let t = (i as f32) / (count as f32) * std::f32::consts::TAU;
            offsets.push((t.cos() * r, t.sin() * r));
        }
    }

    let mut evaluated = 0usize;
    for (dx, dz) in offsets {
        if evaluated >= max {
            break;
        }
        evaluated += 1;
        let candidate_global = Vec3::new(anchor_global.x + dx, anchor_global.y, anchor_global.z + dz);
        let candidate = WorldPosition::from_global(candidate_global, layout);
        let yaw_quadrants = 0u8;
        let rotation = rotation_from_quadrants(yaw_quadrants);
        let validation =
            validate_building_placement(&ctx, definition_id, candidate, rotation, ownership);

        if !validation.valid {
            let reason = validation
                .primary_reason
                .map(|r| r.label().to_string())
                .unwrap_or_else(|| "invalid".into());
            rejected.push(RejectedSiteDiagnostic {
                offset_x: dx,
                offset_z: dz,
                reason,
            });
            continue;
        }

        let grounded = validation
            .grounded_anchor
            .unwrap_or(candidate);
        let soft = soft_preference_score(world, grounded, anchor);
        valid.push(ConstructionPlacementCandidate::from_world_position(
            grounded,
            yaw_quadrants,
            soft,
        ));
    }

    diagnostics.push(format!(
        "placement_search evaluated={evaluated} valid={} rejected={}",
        valid.len(),
        rejected.len()
    ));

    valid.sort_by(|a, b| {
        b.soft_score
            .cmp(&a.soft_score)
            .then_with(|| {
                let ag = a.position().to_global(layout);
                let bg = b.position().to_global(layout);
                ag.x
                    .to_bits()
                    .cmp(&bg.x.to_bits())
                    .then_with(|| ag.z.to_bits().cmp(&bg.z.to_bits()))
            })
    });

    PlacementSearchResult {
        selected: valid.into_iter().next(),
        rejected,
        diagnostics,
    }
}

/// Soft preference only — never used as a hard failure.
fn soft_preference_score(world: &WorldData, site: WorldPosition, anchor: WorldPosition) -> i32 {
    let layout = world.layout();
    let dist = xz_distance(site, anchor, layout);
    // Prefer moderate proximity: close enough for organic clustering, not on top of core.
    let mut score = 500i32;
    if dist < 6.0 {
        score -= 80;
    } else if dist < 20.0 {
        score += 40;
    } else if dist < 40.0 {
        score += 10;
    } else {
        score -= (dist as i32).min(100);
    }

    // Prefer sites with fewer nearby buildings (simple spacing preference).
    let nearby = world
        .sorted_building_ids()
        .into_iter()
        .filter_map(|id| world.get_building(id))
        .filter(|record| {
            let d = xz_distance(record.placement.position, site, layout);
            d < 12.0
        })
        .count();
    score -= (nearby as i32) * 15;
    score
}
