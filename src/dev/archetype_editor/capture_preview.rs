//! Live capture preview for building archetype authoring.

use bevy::prelude::*;

use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::world::{
    BakedCellMask, BuildingArchetypeCaptureRegion, BuildingArchetypeMemberKind, BuildingCatalog,
    DoodadCatalog, FootprintCatalog, FootprintShape, ItemCatalog, WorldConfig, WorldData,
    WorldPosition, capture_building_archetype_members, compute_building_archetype_capture_region,
    default_building_archetype_capture_margin_meters, durable_extensions_summary,
    world_item_member_summary,
};

use super::actions::DevArchetypeEditorScratch;
use super::state::DevArchetypeEditorState;

#[derive(Debug, Clone)]
pub struct BuildingArchetypeMemberPreviewEntry {
    pub kind: BuildingArchetypeMemberKind,
    pub display_name: String,
    pub world_position: Vec3,
    pub state_hint: Option<String>,
    pub stack_quantity: Option<u32>,
}

/// Refresh capture preview scratch state while the building modal is open.
pub fn sync_building_archetype_capture_preview(
    editor: Res<DevArchetypeEditorState>,
    mut scratch: ResMut<DevArchetypeEditorScratch>,
    world: Res<WorldData>,
    building_catalog: Res<BuildingCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    item_catalog: Res<ItemCatalog>,
) {
    if !editor.modal_open || !editor.is_building_modal() {
        if scratch.preview_region.is_some() || !scratch.preview_members.is_empty() {
            scratch.clear_capture_preview();
        }
        return;
    }

    let root = scratch.pending_building.clone();
    let margin_input = scratch.capture_margin_input.clone();
    let Some(root) = root else {
        scratch.clear_capture_preview();
        return;
    };

    let margin = parse_capture_margin_input(&margin_input);
    match capture_building_archetype_members(
        &world,
        &root,
        &building_catalog,
        &footprint_catalog,
        &doodad_catalog,
        margin,
    ) {
        Ok((_, members)) => {
            scratch.preview_region = compute_building_archetype_capture_region(
                &world,
                &root,
                &building_catalog,
                &footprint_catalog,
                margin,
            )
            .ok();
            scratch.preview_members = members
                .iter()
                .map(|member| {
                    let display_name = match member.kind {
                        BuildingArchetypeMemberKind::Building => building_catalog
                            .get(&crate::world::BuildingDefinitionId::new(&member.definition_id))
                            .map(|def| def.display_name.clone())
                            .unwrap_or_else(|| member.definition_id.clone()),
                        BuildingArchetypeMemberKind::Doodad => doodad_catalog
                            .get(&crate::world::DoodadDefinitionId::new(&member.definition_id))
                            .map(|def| def.display_name.clone())
                            .unwrap_or_else(|| member.definition_id.clone()),
                        BuildingArchetypeMemberKind::WorldItemPile => item_catalog
                            .get(&crate::world::ItemDefinitionId::new(&member.definition_id))
                            .map(|def| def.display_name.clone())
                            .unwrap_or_else(|| member.definition_id.clone()),
                    };
                    let world_position = member_world_position(&world, &root, &member.local_pose);
                    let state_hint = match member.kind {
                        BuildingArchetypeMemberKind::Building => member
                            .building_state
                            .as_ref()
                            .and_then(|state| durable_extensions_summary(&state.extensions)),
                        BuildingArchetypeMemberKind::WorldItemPile => member
                            .world_item_state
                            .as_ref()
                            .and_then(world_item_member_summary),
                        BuildingArchetypeMemberKind::Doodad => None,
                    };
                    let stack_quantity = member
                        .world_item_state
                        .as_ref()
                        .and_then(|state| state.stack_quantity);
                    BuildingArchetypeMemberPreviewEntry {
                        kind: member.kind,
                        display_name,
                        world_position,
                        state_hint,
                        stack_quantity,
                    }
                })
                .collect();
        }
        Err(_) => {
            scratch.preview_region = None;
            scratch.preview_members.clear();
        }
    }
}

pub fn draw_building_archetype_capture_preview(
    editor: Res<DevArchetypeEditorState>,
    scratch: Res<DevArchetypeEditorScratch>,
    _world: Res<WorldData>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut gizmos: Gizmos,
) {
    if !editor.modal_open || !editor.is_building_modal() {
        return;
    }
    let Some(region) = scratch.preview_region.as_ref() else {
        return;
    };
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    draw_capture_region_gizmo(&mut gizmos, region, layout, vertical_scale);

    if let Some(root) = scratch.pending_building.as_ref() {
        let root_render = world_position_to_render_global(
            root.placement.position,
            layout,
            vertical_scale,
        );
        gizmos.sphere(
            root_render + Vec3::Y * 0.2,
            0.35,
            Color::srgba(0.2, 0.95, 0.45, 0.9),
        );
    }

    for member in &scratch.preview_members {
        let member_position =
            WorldPosition::from_global(member.world_position, layout);
        let render = world_position_to_render_global(member_position, layout, vertical_scale);
        gizmos.sphere(render + Vec3::Y * 0.15, 0.25, Color::srgba(0.95, 0.8, 0.2, 0.9));
    }
}

pub fn format_captured_member_lines(
    members: &[BuildingArchetypeMemberPreviewEntry],
) -> String {
    if members.is_empty() {
        return "Captured:\n(none)".to_string();
    }
    let mut counts: std::collections::BTreeMap<(BuildingArchetypeMemberKind, String), u32> =
        std::collections::BTreeMap::new();
    for member in members {
        let key = (member.kind, member.display_name.clone());
        counts.entry(key).and_modify(|count| *count += 1).or_insert(1);
    }
    let mut lines = vec!["Captured:".to_string()];
    let mut seen = std::collections::BTreeSet::new();
    for member in members {
        let key = (member.kind, member.display_name.clone());
        if !seen.insert(key.clone()) {
            continue;
        }
        let count = counts.get(&key).copied().unwrap_or(1);
        let suffix = member
            .state_hint
            .as_ref()
            .map(|hint| format!(" — {hint}"))
            .unwrap_or_default();
        if member.kind == BuildingArchetypeMemberKind::WorldItemPile {
            if let Some(quantity) = member.stack_quantity {
                if quantity > 1 {
                    lines.push(format!("- {} x{}{}", member.display_name, quantity, suffix));
                } else {
                    lines.push(format!("- {}{}", member.display_name, suffix));
                }
            } else {
                lines.push(format!("- {}{}", member.display_name, suffix));
            }
            continue;
        }
        if count > 1 {
            lines.push(format!("- {}{} x{}", member.display_name, suffix, count));
        } else {
            lines.push(format!("- {}{}", member.display_name, suffix));
        }
    }
    lines.join("\n")
}

pub fn parse_capture_margin_input(raw: &str) -> f32 {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return default_building_archetype_capture_margin_meters();
    }
    trimmed
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(default_building_archetype_capture_margin_meters())
}

fn member_world_position(
    world: &WorldData,
    root: &crate::world::BuildingRecord,
    local: &crate::world::BuildingArchetypeLocalPose,
) -> Vec3 {
    let layout = world.layout();
    let root_global = root.placement.position.to_global(layout);
    root_global + root.placement.rotation * Vec3::from_array(local.local_position)
}

fn draw_capture_region_gizmo(
    gizmos: &mut Gizmos,
    region: &BuildingArchetypeCaptureRegion,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) {
    let points = region_outline_points(region);
    if points.len() < 2 {
        return;
    }
    let anchor_position = WorldPosition::from_global(
        Vec3::new(region.anchor_xz.x, region.ground_y, region.anchor_xz.y),
        layout,
    );
    let y = world_position_to_render_global(anchor_position, layout, vertical_scale).y + 0.05;
    let color = Color::srgba(0.2, 0.75, 1.0, 0.85);
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        gizmos.line(
            Vec3::new(a.x, y, a.y),
            Vec3::new(b.x, y, b.y),
            color,
        );
    }
}

fn region_outline_points(region: &BuildingArchetypeCaptureRegion) -> Vec<Vec2> {
    match &region.expanded_shape {
        FootprintShape::Circle { radius_meters } => {
            let mut points = Vec::with_capacity(32);
            for i in 0..32 {
                let angle = std::f32::consts::TAU * i as f32 / 32.0;
                points.push(
                    region.anchor_xz
                        + Vec2::new(angle.cos(), angle.sin()) * *radius_meters,
                );
            }
            points
        }
        FootprintShape::Ellipse {
            radius_x_meters,
            radius_z_meters,
        } => {
            let mut points = Vec::with_capacity(32);
            let (sin, cos) = region.yaw_radians.sin_cos();
            for i in 0..32 {
                let angle = std::f32::consts::TAU * i as f32 / 32.0;
                let local = Vec2::new(angle.cos() * *radius_x_meters, angle.sin() * *radius_z_meters);
                points.push(
                    region.anchor_xz
                        + Vec2::new(
                            local.x * cos - local.y * sin,
                            local.x * sin + local.y * cos,
                        ),
                );
            }
            points
        }
        FootprintShape::Rectangle {
            width_meters,
            depth_meters,
        } => oriented_rectangle_points(
            region.anchor_xz,
            *width_meters,
            *depth_meters,
            region.yaw_radians,
        ),
        FootprintShape::BakedCellMask(mask) => {
            baked_mask_outline_points(region.anchor_xz, region.yaw_radians, mask)
        }
    }
}

fn oriented_rectangle_points(
    anchor: Vec2,
    width: f32,
    depth: f32,
    yaw_radians: f32,
) -> Vec<Vec2> {
    let half = Vec2::new(width * 0.5, depth * 0.5);
    let corners = [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(half.x, half.y),
        Vec2::new(-half.x, half.y),
    ];
    let (sin, cos) = yaw_radians.sin_cos();
    corners
        .map(|corner| {
            anchor
                + Vec2::new(
                    corner.x * cos - corner.y * sin,
                    corner.x * sin + corner.y * cos,
                )
        })
        .to_vec()
}

fn baked_mask_outline_points(anchor: Vec2, yaw: f32, mask: &BakedCellMask) -> Vec<Vec2> {
    let cell_size = mask.cell_size_meters;
    let (sin, cos) = yaw.sin_cos();
    let mut points = Vec::new();
    for z in 0..mask.depth_cells {
        for x in 0..mask.width_cells {
            if !mask.is_blocked_local(x as i32, z as i32) {
                continue;
            }
            for corner in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let local = mask.local_origin
                    + Vec2::new((x as f32 + corner.0) * cell_size, (z as f32 + corner.1) * cell_size);
                points.push(
                    anchor
                        + Vec2::new(
                            local.x * cos - local.y * sin,
                            local.x * sin + local.y * cos,
                        ),
                );
            }
        }
    }
    if points.is_empty() {
        points.push(anchor);
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_capture_margin_defaults_for_empty() {
        assert_eq!(
            parse_capture_margin_input(""),
            default_building_archetype_capture_margin_meters()
        );
    }

    #[test]
    fn format_captured_member_lines_groups_duplicates() {
        let lines = format_captured_member_lines(&[
            BuildingArchetypeMemberPreviewEntry {
                kind: BuildingArchetypeMemberKind::Doodad,
                display_name: "Crate".into(),
                world_position: Vec3::ZERO,
                state_hint: None,
                stack_quantity: None,
            },
            BuildingArchetypeMemberPreviewEntry {
                kind: BuildingArchetypeMemberKind::Doodad,
                display_name: "Crate".into(),
                world_position: Vec3::ONE,
                state_hint: None,
                stack_quantity: None,
            },
        ]);
        assert!(lines.contains("Crate x2"));
    }
}
