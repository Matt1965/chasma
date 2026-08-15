//! NAV-ENTRY-D1: test-only ingress path diagnostics (no production behavior changes).

use bevy::prelude::*;

use super::surface_support::resolve_surface_entrance_approach_position;
use crate::world::navigation::{
    NavigationSegmentLegality, is_segment_walkable_in_space,
    resolve_entrance_interior_planning_anchor,
};
use crate::world::{
    NavigationAgent, NavigationConfig, NavigationError, PassabilityCatalogs, PassabilityResult,
    PortalRecord, PortalType, SpaceId, SpaceRegistry, WorldData, WorldPosition,
    find_path_with_spaces, query_navigation_point_legality, query_navigation_segment_legality,
    surface_blueprint_support_blocks_position, surface_position_in_entrance_access_corridor,
};

const MAX_SLOPE: f32 = 45.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngressFailureStage {
    None,
    EntryApproachResolution,
    EntrySurfaceToApproach,
    EntryApproachToPortal,
    EntryInteriorAnchor,
    EntryInteriorFinal,
    EntryFullPathOther,
}

impl IngressFailureStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::EntryApproachResolution => "ENTRY_APPROACH_RESOLUTION",
            Self::EntrySurfaceToApproach => "ENTRY_SURFACE_TO_APPROACH",
            Self::EntryApproachToPortal => "ENTRY_APPROACH_TO_PORTAL",
            Self::EntryInteriorAnchor => "ENTRY_INTERIOR_ANCHOR",
            Self::EntryInteriorFinal => "ENTRY_INTERIOR_FINAL",
            Self::EntryFullPathOther => "ENTRY_FULL_PATH_OTHER",
        }
    }
}

#[derive(Debug)]
pub struct ApproachResolutionProbe {
    pub resolved: bool,
    pub position: Option<WorldPosition>,
    pub position_xz: Option<Vec2>,
    pub point_legality: Option<PassabilityResult>,
    pub inside_support: Option<bool>,
    pub in_access_corridor: Option<bool>,
}

#[derive(Debug)]
pub struct SurfaceLegProbe {
    pub from: WorldPosition,
    pub to: WorldPosition,
    pub direct_legal: bool,
    pub direct_block_reason: Option<String>,
    pub astar_result: Result<u32, NavigationError>,
}

#[derive(Debug)]
pub struct InteriorAnchorProbe {
    pub result: Result<WorldPosition, NavigationError>,
}

#[derive(Debug)]
pub struct InteriorFinalProbe {
    pub from: WorldPosition,
    pub astar_result: Result<u32, NavigationError>,
}

#[derive(Debug)]
pub struct FullPathProbe {
    pub result: Result<u32, NavigationError>,
}

#[derive(Debug)]
pub struct IngressPathDiagnostic {
    pub agent_radius_meters: f32,
    pub start: WorldPosition,
    pub goal: WorldPosition,
    pub start_space: SpaceId,
    pub goal_space: SpaceId,
    pub portal_id: Option<u32>,
    pub portal_enabled: Option<bool>,
    pub portal_trigger: Option<WorldPosition>,
    pub portal_trigger_xz: Option<Vec2>,
    pub approach: ApproachResolutionProbe,
    pub leg1: Option<SurfaceLegProbe>,
    pub leg2: Option<SurfaceLegProbe>,
    pub interior_anchor: Option<InteriorAnchorProbe>,
    pub interior_final: Option<InteriorFinalProbe>,
    pub full_path: FullPathProbe,
    pub first_failure_stage: IngressFailureStage,
    pub first_navigation_error: Option<NavigationError>,
}

fn nav_agent(radius: f32) -> NavigationAgent {
    NavigationAgent {
        radius_meters: radius,
        max_slope_degrees: MAX_SLOPE,
    }
}

fn pass_agent(radius: f32) -> crate::world::PassabilityAgent {
    crate::world::PassabilityAgent {
        radius_meters: radius,
        max_slope_degrees: MAX_SLOPE,
    }
}

fn segment_block_label(legality: NavigationSegmentLegality) -> Option<String> {
    match legality {
        NavigationSegmentLegality::Legal => None,
        NavigationSegmentLegality::Blocked { reason, source } => {
            Some(format!("{reason:?} source={source:?}"))
        }
    }
}

fn probe_surface_leg(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    agent_radius: f32,
    from: WorldPosition,
    to: WorldPosition,
) -> SurfaceLegProbe {
    let layout = world.layout();
    let agent = nav_agent(agent_radius);
    let direct = query_navigation_segment_legality(
        world,
        space_registry,
        catalogs,
        *nav_config,
        SpaceId::SURFACE,
        agent,
        from,
        to,
        layout,
    );
    let direct_legal = is_segment_walkable_in_space(
        world,
        space_registry,
        catalogs,
        *nav_config,
        SpaceId::SURFACE,
        agent,
        from,
        to,
        layout,
    );
    let astar_result = find_path_with_spaces(
        world,
        catalogs,
        nav_config,
        agent_radius,
        MAX_SLOPE,
        from,
        to,
        SpaceId::SURFACE,
        SpaceId::SURFACE,
        None,
    )
    .map(|path| path.len() as u32)
    .map_err(|error| error);
    SurfaceLegProbe {
        from,
        to,
        direct_legal,
        direct_block_reason: segment_block_label(direct),
        astar_result,
    }
}

fn probe_approach_resolution(
    world: &WorldData,
    space_registry: &SpaceRegistry,
    catalogs: PassabilityCatalogs<'_>,
    portal: &PortalRecord,
    agent_radius: f32,
) -> ApproachResolutionProbe {
    let layout = world.layout();
    let position =
        resolve_surface_entrance_approach_position(world, space_registry, portal, agent_radius);
    let Some(position) = position else {
        return ApproachResolutionProbe {
            resolved: false,
            position: None,
            position_xz: None,
            point_legality: None,
            inside_support: None,
            in_access_corridor: None,
        };
    };
    let position_xz = position.to_global(layout).xz();
    let point_legality = query_navigation_point_legality(
        world,
        catalogs,
        position,
        pass_agent(agent_radius),
        SpaceId::SURFACE,
    );
    let inside_support =
        surface_blueprint_support_blocks_position(world, layout, position_xz, agent_radius)
            .is_some();
    let in_access_corridor = portal.owning_building_id.and_then(|building_id| {
        world
            .building_navigation_runtime()
            .get(building_id)
            .map(|runtime| {
                surface_position_in_entrance_access_corridor(
                    space_registry,
                    layout,
                    building_id,
                    &runtime.regions[0],
                    position_xz,
                    agent_radius,
                )
            })
    });
    ApproachResolutionProbe {
        resolved: true,
        position: Some(position),
        position_xz: Some(position_xz),
        point_legality: Some(point_legality),
        inside_support: Some(inside_support),
        in_access_corridor,
    }
}

pub fn diagnose_surface_to_interior_ingress(
    world: &WorldData,
    catalogs: PassabilityCatalogs<'_>,
    nav_config: &NavigationConfig,
    agent_radius: f32,
    start: WorldPosition,
    goal: WorldPosition,
    start_space: SpaceId,
    goal_space: SpaceId,
    portal: &PortalRecord,
) -> IngressPathDiagnostic {
    let layout = world.layout();
    let space_registry = world.space_registry();
    let portal_trigger =
        portal.trigger_world_position_in_space(start_space, layout, world, space_registry);
    let approach = probe_approach_resolution(world, space_registry, catalogs, portal, agent_radius);

    let mut first_failure_stage = IngressFailureStage::None;
    let mut first_navigation_error = None;

    let leg1 = if approach.resolved {
        let approach_pos = approach.position.expect("resolved approach");
        Some(probe_surface_leg(
            world,
            space_registry,
            catalogs,
            nav_config,
            agent_radius,
            start,
            approach_pos,
        ))
    } else {
        first_failure_stage = IngressFailureStage::EntryApproachResolution;
        None
    };

    if first_failure_stage == IngressFailureStage::None {
        if let Some(leg) = &leg1 {
            if leg.astar_result.is_err() {
                first_failure_stage = IngressFailureStage::EntrySurfaceToApproach;
                first_navigation_error = leg.astar_result.err();
            }
        }
    }

    let leg2 = if first_failure_stage == IngressFailureStage::None {
        let approach_pos = approach.position.expect("approach");
        let portal_entry = portal_trigger.expect("portal trigger in start space");
        Some(probe_surface_leg(
            world,
            space_registry,
            catalogs,
            nav_config,
            agent_radius,
            approach_pos,
            portal_entry,
        ))
    } else {
        None
    };

    if first_failure_stage == IngressFailureStage::None {
        if let Some(leg) = &leg2 {
            if leg.astar_result.is_err() {
                first_failure_stage = IngressFailureStage::EntryApproachToPortal;
                first_navigation_error = leg.astar_result.err();
            }
        }
    }

    let interior_anchor = if first_failure_stage == IngressFailureStage::None
        && portal.portal_type == PortalType::ExteriorEntrance
    {
        let agent = nav_agent(agent_radius);
        let dest_space = portal.to_space;
        let anchor = resolve_entrance_interior_planning_anchor(
            world,
            space_registry,
            catalogs,
            portal,
            dest_space,
            layout,
            *nav_config,
            agent,
        )
        .ok_or(NavigationError::StartBlocked);
        if anchor.is_err() {
            first_failure_stage = IngressFailureStage::EntryInteriorAnchor;
            first_navigation_error = anchor.err();
        }
        Some(InteriorAnchorProbe { result: anchor })
    } else {
        None
    };

    let interior_final = if first_failure_stage == IngressFailureStage::None {
        if let Some(InteriorAnchorProbe { result: Ok(anchor) }) = interior_anchor.as_ref() {
            let result = find_path_with_spaces(
                world,
                catalogs,
                nav_config,
                agent_radius,
                MAX_SLOPE,
                *anchor,
                goal,
                portal.to_space,
                goal_space,
                None,
            )
            .map(|path| path.len() as u32);
            if result.is_err() {
                first_failure_stage = IngressFailureStage::EntryInteriorFinal;
                first_navigation_error = result.err();
            }
            Some(InteriorFinalProbe {
                from: *anchor,
                astar_result: result,
            })
        } else {
            None
        }
    } else {
        None
    };

    let full_path = FullPathProbe {
        result: find_path_with_spaces(
            world,
            catalogs,
            nav_config,
            agent_radius,
            MAX_SLOPE,
            start,
            goal,
            start_space,
            goal_space,
            None,
        )
        .map(|path| path.len() as u32)
        .map_err(|error| error),
    };

    if first_failure_stage == IngressFailureStage::None {
        if full_path.result.is_err() {
            first_failure_stage = IngressFailureStage::EntryFullPathOther;
            first_navigation_error = full_path.result.err();
        }
    }

    IngressPathDiagnostic {
        agent_radius_meters: agent_radius,
        start,
        goal,
        start_space,
        goal_space,
        portal_id: Some(portal.id.raw()),
        portal_enabled: Some(portal.enabled),
        portal_trigger,
        portal_trigger_xz: portal_trigger.map(|pos| pos.to_global(layout).xz()),
        approach,
        leg1,
        leg2,
        interior_anchor,
        interior_final,
        full_path,
        first_failure_stage,
        first_navigation_error,
    }
}

pub fn format_ingress_diagnostic(diagnostic: &IngressPathDiagnostic) -> String {
    format!("{diagnostic:#?}")
}
