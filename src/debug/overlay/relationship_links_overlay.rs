//! Relationship link debug overlay — mutual perception + directional totals (ADR-132 dev).

use bevy::camera::Camera;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::camera::RtsCamera;
use crate::debug::relationship_links::{
    RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS, discover_mutual_perception_relationship_links,
    format_signed_relationship,
};
use crate::debug::settings::{DebugOverlayCategory, DebugOverlaySettings};
use crate::terrain::TerrainRenderAssets;
use crate::units::input::world_position_to_screen;
use crate::world::{AuthoredRelationshipCatalog, UnitCatalog, WorldConfig, WorldData};

use super::helpers::{render_position, xz_to_render_y};

const A_TO_B_LINE_COLOR: Color = Color::srgba(0.25, 0.85, 1.0, 0.92);
const B_TO_A_LINE_COLOR: Color = Color::srgba(1.0, 0.55, 0.15, 0.92);
const LABEL_Y_OFFSET: f32 = 0.8;
const LABEL_SCREEN_OFFSET: Vec2 = Vec2::new(16.0, 8.0);

/// Full-screen pass-through root for projected relationship link labels.
#[derive(Component, Debug)]
pub struct RelationshipLinkLabelsRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RelationshipLinkLabelKey {
    unit_a: crate::world::UnitId,
    unit_b: crate::world::UnitId,
    /// `false` = A→B label on the A-side half; `true` = B→A label on the B-side half.
    b_side: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct RelationshipLinkLabelAnchor {
    world: Vec3,
}

#[derive(Resource, Default, Debug)]
pub struct RelationshipLinkLabelIndex(std::collections::HashMap<RelationshipLinkLabelKey, Entity>);

/// Spawn the screen-space label overlay root once at startup.
pub fn setup_relationship_link_labels_overlay(mut commands: Commands) {
    commands.spawn((
        RelationshipLinkLabelsRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        FocusPolicy::Pass,
        ZIndex(250),
        Visibility::Visible,
    ));
}

pub fn draw_relationship_links_overlay(
    mut gizmos: Gizmos,
    mut commands: Commands,
    settings: Res<DebugOverlaySettings>,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    authored: Res<AuthoredRelationshipCatalog>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut label_index: ResMut<RelationshipLinkLabelIndex>,
    labels: Query<(Entity, &RelationshipLinkLabelKey)>,
    root: Query<Entity, With<RelationshipLinkLabelsRoot>>,
) {
    if !settings.category_enabled(DebugOverlayCategory::RelationshipLinks) {
        for (entity, _) in &labels {
            commands.entity(entity).despawn();
        }
        label_index.0.clear();
        return;
    }

    let Ok(root) = root.single() else {
        return;
    };

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let standing = world.relationship_standing_store();
    let pairs = discover_mutual_perception_relationship_links(
        &world,
        &catalog,
        &authored,
        standing,
        RELATIONSHIP_LINK_VIZ_MAX_DISTANCE_METERS,
    );

    let mut desired = std::collections::HashSet::new();
    for pair in &pairs {
        let Some(record_a) = world.get_unit(pair.unit_a) else {
            continue;
        };
        let Some(record_b) = world.get_unit(pair.unit_b) else {
            continue;
        };

        let pos_a = render_position(record_a.placement.position, layout, vertical_scale);
        let pos_b = render_position(record_b.placement.position, layout, vertical_scale);
        let midpoint = (pos_a + pos_b) * 0.5;

        gizmos.line(
            xz_to_render_y(pos_a, 0.12),
            xz_to_render_y(midpoint, 0.12),
            A_TO_B_LINE_COLOR,
        );
        gizmos.line(
            xz_to_render_y(midpoint, 0.12),
            xz_to_render_y(pos_b, 0.12),
            B_TO_A_LINE_COLOR,
        );

        let label_a_anchor =
            relationship_link_label_world_anchor(pos_a, pos_b, false, LABEL_Y_OFFSET);
        let label_b_anchor =
            relationship_link_label_world_anchor(pos_a, pos_b, true, LABEL_Y_OFFSET);
        sync_label(
            &mut commands,
            root,
            &mut label_index,
            RelationshipLinkLabelKey {
                unit_a: pair.unit_a,
                unit_b: pair.unit_b,
                b_side: false,
            },
            label_a_anchor,
            &format_signed_relationship(pair.a_to_b),
            A_TO_B_LINE_COLOR,
        );
        sync_label(
            &mut commands,
            root,
            &mut label_index,
            RelationshipLinkLabelKey {
                unit_a: pair.unit_a,
                unit_b: pair.unit_b,
                b_side: true,
            },
            label_b_anchor,
            &format_signed_relationship(pair.b_to_a),
            B_TO_A_LINE_COLOR,
        );
        desired.insert(RelationshipLinkLabelKey {
            unit_a: pair.unit_a,
            unit_b: pair.unit_b,
            b_side: false,
        });
        desired.insert(RelationshipLinkLabelKey {
            unit_a: pair.unit_a,
            unit_b: pair.unit_b,
            b_side: true,
        });
    }

    let stale: Vec<RelationshipLinkLabelKey> = label_index
        .0
        .keys()
        .copied()
        .filter(|key| !desired.contains(key))
        .collect();
    for key in stale {
        if let Some(entity) = label_index.0.remove(&key) {
            commands.entity(entity).despawn();
        }
    }
}

/// Project world-space label anchors through the active RTS camera each frame.
pub fn project_relationship_link_label_positions(
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    mut labels: Query<
        (&RelationshipLinkLabelAnchor, &mut Node, &mut Visibility),
        With<RelationshipLinkLabelKey>,
    >,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        for (_, mut node, mut visibility) in &mut labels {
            sync_label_screen_node(None, LABEL_SCREEN_OFFSET, &mut node, &mut visibility);
        }
        return;
    };

    for (anchor, mut node, mut visibility) in &mut labels {
        let projection = world_position_to_screen(anchor.world, camera, camera_transform);
        sync_label_screen_node(projection, LABEL_SCREEN_OFFSET, &mut node, &mut visibility);
    }
}

pub(crate) fn relationship_link_label_world_anchor(
    pos_a: Vec3,
    pos_b: Vec3,
    b_side: bool,
    y_offset: f32,
) -> Vec3 {
    let t = if b_side { 0.75 } else { 0.25 };
    xz_to_render_y(pos_a.lerp(pos_b, t), y_offset)
}

pub(crate) fn sync_label_screen_node(
    projection: Option<Vec2>,
    offset: Vec2,
    node: &mut Node,
    visibility: &mut Visibility,
) {
    match projection {
        Some(screen) => {
            node.left = Val::Px(screen.x - offset.x);
            node.top = Val::Px(screen.y - offset.y);
            node.display = Display::Flex;
            *visibility = Visibility::Visible;
        }
        None => {
            *visibility = Visibility::Hidden;
            node.display = Display::None;
        }
    }
}

fn sync_label(
    commands: &mut Commands,
    root: Entity,
    label_index: &mut RelationshipLinkLabelIndex,
    key: RelationshipLinkLabelKey,
    world_anchor: Vec3,
    text: &str,
    color: Color,
) {
    if let Some(entity) = label_index.0.get(&key).copied() {
        commands.entity(entity).insert((
            RelationshipLinkLabelAnchor {
                world: world_anchor,
            },
            Text::new(text),
            TextColor(color),
        ));
        return;
    }

    let entity = commands
        .spawn((
            key,
            RelationshipLinkLabelAnchor {
                world: world_anchor,
            },
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                ..default()
            },
            Text::new(text),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(color),
            FocusPolicy::Pass,
            Visibility::Hidden,
            ChildOf(root),
        ))
        .id();
    label_index.0.insert(key, entity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_anchor_positions_at_quarter_and_three_quarter() {
        let pos_a = Vec3::new(0.0, 0.0, 0.0);
        let pos_b = Vec3::new(100.0, 0.0, 0.0);
        let a_side = relationship_link_label_world_anchor(pos_a, pos_b, false, 0.8);
        let b_side = relationship_link_label_world_anchor(pos_a, pos_b, true, 0.8);
        assert_eq!(a_side, Vec3::new(25.0, 0.8, 0.0));
        assert_eq!(b_side, Vec3::new(75.0, 0.8, 0.0));
    }

    #[test]
    fn a_side_uses_near_unit_b_side_uses_far_unit_along_segment() {
        let pos_a = Vec3::new(10.0, 0.0, 20.0);
        let pos_b = Vec3::new(50.0, 0.0, 60.0);
        let a_side = relationship_link_label_world_anchor(pos_a, pos_b, false, 0.0);
        let b_side = relationship_link_label_world_anchor(pos_a, pos_b, true, 0.0);
        assert!((a_side - pos_a).length() < (a_side - pos_b).length());
        assert!((b_side - pos_b).length() < (b_side - pos_a).length());
    }

    #[test]
    fn projection_failure_hides_label_node() {
        let mut node = Node {
            position_type: PositionType::Absolute,
            display: Display::Flex,
            ..default()
        };
        let mut visibility = Visibility::Visible;
        sync_label_screen_node(None, LABEL_SCREEN_OFFSET, &mut node, &mut visibility);
        assert_eq!(node.display, Display::None);
        assert_eq!(visibility, Visibility::Hidden);
    }

    #[test]
    fn projection_success_positions_label_node() {
        let mut node = Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            ..default()
        };
        let mut visibility = Visibility::Hidden;
        sync_label_screen_node(
            Some(Vec2::new(200.0, 100.0)),
            LABEL_SCREEN_OFFSET,
            &mut node,
            &mut visibility,
        );
        assert_eq!(node.left, Val::Px(200.0 - LABEL_SCREEN_OFFSET.x));
        assert_eq!(node.top, Val::Px(100.0 - LABEL_SCREEN_OFFSET.y));
        assert_eq!(node.display, Display::Flex);
        assert_eq!(visibility, Visibility::Visible);
    }

    #[test]
    fn stale_label_keys_are_removed_from_index() {
        let key_a = RelationshipLinkLabelKey {
            unit_a: crate::world::UnitId::new(1),
            unit_b: crate::world::UnitId::new(2),
            b_side: false,
        };
        let key_b = RelationshipLinkLabelKey {
            unit_a: crate::world::UnitId::new(1),
            unit_b: crate::world::UnitId::new(2),
            b_side: true,
        };
        let mut index = RelationshipLinkLabelIndex::default();
        index.0.insert(key_a, Entity::from_bits(1));
        index.0.insert(key_b, Entity::from_bits(2));

        let desired = std::collections::HashSet::from([key_a]);
        let stale: Vec<_> = index
            .0
            .keys()
            .copied()
            .filter(|key| !desired.contains(key))
            .collect();
        for key in stale {
            index.0.remove(&key);
        }

        assert!(index.0.contains_key(&key_a));
        assert!(!index.0.contains_key(&key_b));
    }
}
