//! Doodad inspector snapshot (ADR-098 DT2).

use bevy::prelude::*;

use crate::world::{
    DoodadCatalog, DoodadId, FootprintCatalog, WorldData, doodad_final_render_scale,
    occupied_cells_for_footprint_yaw, resolve_doodad_collision, tilted_blocker_projection_warning,
};

#[derive(Debug, Clone, PartialEq)]
pub struct DoodadInspectorSnapshot {
    pub doodad_id: DoodadId,
    pub definition_id: String,
    pub position: Vec3,
    pub rotation_deg: Vec3,
    pub scale: Vec3,
    pub visual_size: Vec3,
    pub collision_shape: String,
    pub occupied_cell_count: usize,
    pub tilt_warning: Option<String>,
}

pub fn capture_doodad_inspector_snapshot(
    world: &WorldData,
    catalog: &DoodadCatalog,
    _footprint: &FootprintCatalog,
    doodad_id: DoodadId,
) -> Option<DoodadInspectorSnapshot> {
    let record = world.get_doodad(doodad_id)?;
    let definition = catalog.get(&record.definition_id)?;
    let collision = resolve_doodad_collision(record, definition);
    let global = record.placement.position.to_global(world.layout());
    let instance_scale = record.placement.scale_vec3();
    let final_scale = doodad_final_render_scale(definition, instance_scale);
    let cell_count = if collision.blocks_movement {
        occupied_cells_for_footprint_yaw(
            &collision.shape,
            Vec2::new(global.x, global.z),
            collision.yaw_radians,
        )
        .len()
    } else {
        0
    };

    Some(DoodadInspectorSnapshot {
        doodad_id,
        definition_id: record.definition_id.as_str().to_string(),
        position: global,
        rotation_deg: Vec3::new(
            record.placement.orientation.pitch_degrees(),
            record.placement.orientation.yaw_degrees(),
            record.placement.orientation.roll_degrees(),
        ),
        scale: instance_scale,
        visual_size: final_scale,
        collision_shape: format!("{:?}", collision.shape),
        occupied_cell_count: cell_count,
        tilt_warning: tilted_blocker_projection_warning(record),
    })
}
