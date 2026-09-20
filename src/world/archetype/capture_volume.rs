//! Spatial capture region and member query for building archetypes.

use bevy::prelude::*;

use crate::world::building::BuildingRecord;
use crate::world::{
    BakedCellMask, BuildingCatalog, BuildingDefinitionId, BuildingId, ChunkCoord, ChunkId,
    ChunkLayout, DoodadCatalog, DoodadDefinitionId, DoodadId, DoodadRecord, FootprintCatalog,
    FootprintShape, OccupancyError, WorldData, WorldPosition,
    effective_building_footprint_for_placement, point_in_oriented_rectangle_continuous,
};

use super::building::{
    BuildingArchetypeLocalPose, BuildingArchetypeMember, BuildingArchetypeMemberKind,
};
use super::durable_capture::capture_building_member_building_state;

/// Oriented capture region derived from a root building footprint plus margin.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingArchetypeCaptureRegion {
    pub anchor_xz: Vec2,
    pub yaw_radians: f32,
    pub expanded_shape: FootprintShape,
    pub ground_y: f32,
    pub margin_meters: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BuildingArchetypeCaptureError {
    RootBuildingNotFound(BuildingDefinitionId),
    Footprint(OccupancyError),
}

/// Compute the expanded oriented capture region for a root building instance.
pub fn compute_building_archetype_capture_region(
    world: &WorldData,
    root: &BuildingRecord,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    capture_margin_meters: f32,
) -> Result<BuildingArchetypeCaptureRegion, BuildingArchetypeCaptureError> {
    let definition = building_catalog
        .get(&root.definition_id)
        .ok_or_else(|| BuildingArchetypeCaptureError::RootBuildingNotFound(root.definition_id.clone()))?;
    let base_shape = effective_building_footprint_for_placement(
        definition,
        footprint_catalog,
        root.placement.uniform_scale_f32(),
    )
    .map_err(BuildingArchetypeCaptureError::Footprint)?;
    let layout = world.layout();
    let anchor_global = root.placement.position.to_global(layout);
    Ok(BuildingArchetypeCaptureRegion {
        anchor_xz: Vec2::new(anchor_global.x, anchor_global.z),
        yaw_radians: root.placement.rotation.to_euler(EulerRot::YXZ).0,
        expanded_shape: expand_footprint_shape(base_shape.as_ref(), capture_margin_meters),
        ground_y: anchor_global.y,
        margin_meters: capture_margin_meters,
    })
}

/// Query spatial members for a building archetype capture (deterministic order).
pub fn query_building_archetype_members(
    world: &WorldData,
    region: &BuildingArchetypeCaptureRegion,
    root_id: BuildingId,
    building_catalog: &BuildingCatalog,
    doodad_catalog: &DoodadCatalog,
) -> Vec<BuildingArchetypeMember> {
    let layout = world.layout();
    let root = world.get_building(root_id);
    let mut members = Vec::new();

    let (min_xz, max_xz) = footprint_world_aabb(&region.expanded_shape, region.anchor_xz, region.yaw_radians);
    for chunk_coord in chunks_intersecting_aabb(min_xz, max_xz, layout) {
        let chunk_id = ChunkId::new(chunk_coord);
        if let Some(store) = world.buildings_in_chunk(chunk_id) {
            for record in store.records() {
                if record.id == root_id {
                    continue;
                }
                if pivot_in_capture_region(record.placement.position, layout, region) {
                    members.push(capture_building_member(
                        world,
                        root.expect("root exists"),
                        record,
                        layout,
                    ));
                }
            }
        }
        if let Some(store) = world.doodads_in_chunk(chunk_id) {
            for record in store.records() {
                if pivot_in_capture_region(record.placement.position, layout, region) {
                    members.push(capture_doodad_member(root.expect("root exists"), record, layout));
                }
            }
        }
    }

    members.sort_by(|a, b| member_sort_key(a, building_catalog, doodad_catalog).cmp(&member_sort_key(b, building_catalog, doodad_catalog)));
    members
}

fn member_sort_key(
    member: &BuildingArchetypeMember,
    building_catalog: &BuildingCatalog,
    doodad_catalog: &DoodadCatalog,
) -> (u8, String, [i32; 3]) {
    let kind_order = match member.kind {
        BuildingArchetypeMemberKind::Building => 0,
        BuildingArchetypeMemberKind::Doodad => 1,
    };
    let definition = member.definition_id.clone();
    let position_key = [
        (member.local_pose.local_position[0] * 1000.0) as i32,
        (member.local_pose.local_position[1] * 1000.0) as i32,
        (member.local_pose.local_position[2] * 1000.0) as i32,
    ];
    let _ = (building_catalog, doodad_catalog);
    (kind_order, definition, position_key)
}

fn capture_building_member(
    world: &WorldData,
    root: &BuildingRecord,
    member: &BuildingRecord,
    layout: ChunkLayout,
) -> BuildingArchetypeMember {
    let member_global = member.placement.position.to_global(layout);
    let local_pose = compute_local_pose(
        layout,
        root,
        member_global,
        member.placement.rotation,
        Some(member.placement.uniform_scale_f32()),
        None,
    );
    BuildingArchetypeMember {
        kind: BuildingArchetypeMemberKind::Building,
        definition_id: member.definition_id.as_str().to_string(),
        local_pose,
        building_state: Some(capture_building_member_building_state(world, member)),
    }
}

fn capture_doodad_member(
    root: &BuildingRecord,
    member: &DoodadRecord,
    layout: ChunkLayout,
) -> BuildingArchetypeMember {
    let member_global = member.placement.position.to_global(layout);
    let local_pose = compute_local_pose(
        layout,
        root,
        member_global,
        member.placement.rotation_quat(),
        None,
        Some(member.placement.scale_vec3()),
    );
    BuildingArchetypeMember {
        kind: BuildingArchetypeMemberKind::Doodad,
        definition_id: member.definition_id.as_str().to_string(),
        local_pose,
        building_state: None,
    }
}

pub fn compute_local_pose(
    layout: ChunkLayout,
    root: &BuildingRecord,
    member_world: Vec3,
    member_rotation: Quat,
    member_uniform_scale: Option<f32>,
    member_doodad_scale: Option<Vec3>,
) -> BuildingArchetypeLocalPose {
    let root_global = root.placement.position.to_global(layout);
    let root_rotation = root.placement.rotation;
    let inv_root = root_rotation.inverse();
    let local_position = inv_root * (member_world - root_global);
    let local_rotation = (inv_root * member_rotation).normalize();

    let (uniform_scale_milli, scale_x_milli, scale_y_milli, scale_z_milli) =
        if let Some(uniform) = member_uniform_scale {
            let milli = (uniform * 1000.0).round() as i32;
            (milli, 0, 0, 0)
        } else if let Some(scale) = member_doodad_scale {
            (
                0,
                (scale.x * 1000.0).round() as i32,
                (scale.y * 1000.0).round() as i32,
                (scale.z * 1000.0).round() as i32,
            )
        } else {
            (1000, 0, 0, 0)
        };

    BuildingArchetypeLocalPose {
        local_position: [local_position.x, local_position.y, local_position.z],
        local_rotation: [
            local_rotation.x,
            local_rotation.y,
            local_rotation.z,
            local_rotation.w,
        ],
        uniform_scale_milli,
        scale_x_milli,
        scale_y_milli,
        scale_z_milli,
    }
}

pub fn pivot_in_capture_region(
    position: WorldPosition,
    layout: ChunkLayout,
    region: &BuildingArchetypeCaptureRegion,
) -> bool {
    let global = position.to_global(layout);
    let point = Vec2::new(global.x, global.z);
    point_in_expanded_footprint(
        point,
        region.anchor_xz,
        region.yaw_radians,
        &region.expanded_shape,
        region.margin_meters,
    )
}

pub fn expand_footprint_shape(shape: &FootprintShape, margin_meters: f32) -> FootprintShape {
    if margin_meters <= 0.0 {
        return shape.clone();
    }
    match shape {
        FootprintShape::Circle { radius_meters } => FootprintShape::Circle {
            radius_meters: radius_meters + margin_meters,
        },
        FootprintShape::Ellipse {
            radius_x_meters,
            radius_z_meters,
        } => FootprintShape::Ellipse {
            radius_x_meters: radius_x_meters + margin_meters,
            radius_z_meters: radius_z_meters + margin_meters,
        },
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => FootprintShape::Rectangle {
            width_meters: width_meters + 2.0 * margin_meters,
            depth_meters: depth_meters + 2.0 * margin_meters,
        },
        FootprintShape::BakedCellMask(mask) => FootprintShape::BakedCellMask(mask.clone()),
    }
}

pub fn point_in_expanded_footprint(
    point: Vec2,
    anchor: Vec2,
    yaw_radians: f32,
    shape: &FootprintShape,
    margin_meters: f32,
) -> bool {
    match shape {
        FootprintShape::Circle { radius_meters } => {
            (point - anchor).length_squared() <= radius_meters * radius_meters
        }
        FootprintShape::Ellipse {
            radius_x_meters,
            radius_z_meters,
        } => {
            let (sin, cos) = yaw_radians.sin_cos();
            let local = point - anchor;
            let lx = local.x * cos + local.y * sin;
            let lz = -local.x * sin + local.y * cos;
            (lx * lx) / (radius_x_meters * radius_x_meters)
                + (lz * lz) / (radius_z_meters * radius_z_meters)
                <= 1.0
        }
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => point_in_oriented_rectangle_continuous(
            point,
            anchor,
            *width_meters,
            *depth_meters,
            yaw_radians,
        ),
        FootprintShape::BakedCellMask(mask) => {
            point_in_baked_mask(point, anchor, yaw_radians, mask)
                || point_near_baked_mask(point, anchor, yaw_radians, mask, margin_meters)
        }
    }
}

fn point_in_baked_mask(point: Vec2, anchor: Vec2, yaw: f32, mask: &BakedCellMask) -> bool {
    let (sin, cos) = yaw.sin_cos();
    let local = point - anchor;
    let lx = local.x * cos + local.y * sin;
    let lz = -local.x * sin + local.y * cos;
    let lx = lx - mask.local_origin.x;
    let lz = lz - mask.local_origin.y;
    if lx < 0.0 || lz < 0.0 {
        return false;
    }
    let cx = (lx / mask.cell_size_meters).floor() as i32;
    let cz = (lz / mask.cell_size_meters).floor() as i32;
    mask.is_blocked_local(cx, cz)
}

fn point_near_baked_mask(
    point: Vec2,
    anchor: Vec2,
    yaw: f32,
    mask: &BakedCellMask,
    margin_meters: f32,
) -> bool {
    let cell_size = mask.cell_size_meters;
    let (sin, cos) = yaw.sin_cos();
    for z in 0..mask.depth_cells {
        for x in 0..mask.width_cells {
            if !mask.is_blocked_local(x as i32, z as i32) {
                continue;
            }
            let local = mask.local_origin
                + Vec2::new((x as f32 + 0.5) * cell_size, (z as f32 + 0.5) * cell_size);
            let world = anchor
                + Vec2::new(
                    local.x * cos - local.y * sin,
                    local.x * sin + local.y * cos,
                );
            let half = cell_size * 0.5 + margin_meters;
            let delta = (point - world).abs();
            if delta.x <= half && delta.y <= half {
                return true;
            }
        }
    }
    false
}

pub fn footprint_world_aabb(shape: &FootprintShape, anchor: Vec2, yaw_radians: f32) -> (Vec2, Vec2) {
    let corners = footprint_corners(shape, anchor, yaw_radians);
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for corner in corners {
        min = min.min(corner);
        max = max.max(corner);
    }
    (min, max)
}

fn footprint_corners(shape: &FootprintShape, anchor: Vec2, yaw_radians: f32) -> Vec<Vec2> {
    match shape {
        FootprintShape::Circle { radius_meters } => {
            let mut corners = Vec::with_capacity(16);
            for i in 0..16 {
                let angle = std::f32::consts::TAU * i as f32 / 16.0;
                corners.push(anchor + Vec2::new(angle.cos(), angle.sin()) * *radius_meters);
            }
            corners
        }
        FootprintShape::Ellipse {
            radius_x_meters,
            radius_z_meters,
        } => {
            let mut corners = Vec::with_capacity(16);
            for i in 0..16 {
                let angle = std::f32::consts::TAU * i as f32 / 16.0;
                let local = Vec2::new(angle.cos() * *radius_x_meters, angle.sin() * *radius_z_meters);
                corners.push(anchor + rotate_local_xz_vec(local, yaw_radians));
            }
            corners
        }
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => oriented_rectangle_corners(anchor, *width_meters, *depth_meters, yaw_radians),
        FootprintShape::BakedCellMask(mask) => {
            let cell_size = mask.cell_size_meters;
            let mut corners = Vec::new();
            let (sin, cos) = yaw_radians.sin_cos();
            for z in 0..mask.depth_cells {
                for x in 0..mask.width_cells {
                    if !mask.is_blocked_local(x as i32, z as i32) {
                        continue;
                    }
                    let local = mask.local_origin
                        + Vec2::new(x as f32 * cell_size, z as f32 * cell_size);
                    corners.push(
                        anchor
                            + Vec2::new(
                                local.x * cos - local.y * sin,
                                local.x * sin + local.y * cos,
                            ),
                    );
                    let local = mask.local_origin
                        + Vec2::new((x as f32 + 1.0) * cell_size, (z as f32 + 1.0) * cell_size);
                    corners.push(
                        anchor
                            + Vec2::new(
                                local.x * cos - local.y * sin,
                                local.x * sin + local.y * cos,
                            ),
                    );
                }
            }
            if corners.is_empty() {
                corners.push(anchor);
            }
            corners
        }
    }
}

fn oriented_rectangle_corners(
    anchor: Vec2,
    width: f32,
    depth: f32,
    yaw_radians: f32,
) -> Vec<Vec2> {
    let half = Vec2::new(width * 0.5, depth * 0.5);
    [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(half.x, half.y),
        Vec2::new(-half.x, half.y),
    ]
    .map(|corner| anchor + rotate_local_xz_vec(corner, yaw_radians))
    .to_vec()
}

fn rotate_local_xz_vec(local: Vec2, yaw_radians: f32) -> Vec2 {
    let (sin, cos) = yaw_radians.sin_cos();
    Vec2::new(
        local.x * cos - local.y * sin,
        local.x * sin + local.y * cos,
    )
}

fn chunks_intersecting_aabb(min_xz: Vec2, max_xz: Vec2, layout: ChunkLayout) -> Vec<ChunkCoord> {
    let size = layout.chunk_size_units();
    let min_cx = (min_xz.x / size).floor() as i32;
    let max_cx = (max_xz.x / size).floor() as i32;
    let min_cz = (min_xz.y / size).floor() as i32;
    let max_cz = (max_xz.y / size).floor() as i32;
    let mut chunks = Vec::new();
    for cz in min_cz..=max_cz {
        for cx in min_cx..=max_cx {
            chunks.push(ChunkCoord::new(cx, cz));
        }
    }
    chunks
}

