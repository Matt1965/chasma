use bevy::prelude::*;

use crate::world::SettlementAnchorId;

use super::components::SettlementAnchorRenderEntity;

#[derive(Resource, Default, Debug)]
pub struct SettlementAnchorRenderIndex(pub std::collections::HashMap<SettlementAnchorId, Entity>);

pub fn despawn_settlement_anchor_render_entities(
    commands: &mut Commands,
    index: &mut SettlementAnchorRenderIndex,
    stale: Vec<SettlementAnchorId>,
) {
    for anchor_id in stale {
        if let Some(entity) = index.0.remove(&anchor_id) {
            commands.entity(entity).despawn();
        }
    }
}

pub fn spawn_settlement_anchor_render_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    index: &mut SettlementAnchorRenderIndex,
    anchor_id: SettlementAnchorId,
    global: Vec3,
) -> Entity {
    let entity = commands
        .spawn((
            SettlementAnchorRenderEntity { anchor_id },
            Mesh3d(meshes.add(Cylinder::new(0.35, 2.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(0.95, 0.82, 0.2, 0.9),
                emissive: LinearRgba::new(0.4, 0.3, 0.05, 1.0),
                ..default()
            })),
            Transform::from_translation(global),
            Name::new(format!("SettlementAnchor#{}", anchor_id.raw())),
        ))
        .id();
    index.0.insert(anchor_id, entity);
    entity
}
