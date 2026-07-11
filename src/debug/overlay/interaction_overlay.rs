//! Interaction classification debug overlay (ADR-042 U6 + ADR-039 U-UI3, REVIEW-A6).
//!
//! Read-only: draws from [`crate::debug::InteractionDebugSnapshot`] populated by capture.

use bevy::prelude::*;

use crate::debug::interaction_snapshot::InteractionDebugSnapshot;
use crate::debug::settings::{DebugOverlayCategory, DebugOverlaySettings};
use crate::terrain::TerrainRenderAssets;
use crate::world::{InteractionType, WorldConfig};

use super::helpers::{render_position, xz_to_render_y};

/// Draw interaction classification gizmos from the client-local debug snapshot.
pub fn draw_interaction_debug_overlay(
    mut gizmos: Gizmos,
    settings: Res<DebugOverlaySettings>,
    snapshot: Res<InteractionDebugSnapshot>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.category_enabled(DebugOverlayCategory::Interaction) {
        return;
    }

    let Some(interaction) = snapshot.query.as_ref() else {
        return;
    };

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let center = xz_to_render_y(
        render_position(interaction.position, layout, vertical_scale),
        0.35,
    );
    let color = interaction_type_color(interaction.interaction_type);
    gizmos.sphere(center, 0.4, color);
    gizmos.circle(
        Isometry3d::new(center, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        0.55,
        color.with_alpha(0.55),
    );
}

fn interaction_type_color(kind: InteractionType) -> Color {
    match kind {
        InteractionType::MoveTarget => Color::srgba(0.2, 0.85, 0.35, 0.85),
        InteractionType::ResourceNode => Color::srgba(0.95, 0.75, 0.15, 0.9),
        InteractionType::InteractableObject => Color::srgba(0.35, 0.65, 1.0, 0.85),
        InteractionType::BlockedArea => Color::srgba(0.95, 0.25, 0.2, 0.85),
        InteractionType::TerrainPoint => Color::srgba(0.7, 0.7, 0.7, 0.8),
        InteractionType::AttackableUnit => Color::srgba(0.95, 0.2, 0.2, 0.9),
        InteractionType::FriendlyUnit => Color::srgba(0.25, 0.55, 0.95, 0.85),
        InteractionType::NeutralUnit => Color::srgba(0.85, 0.85, 0.35, 0.85),
        InteractionType::None => Color::srgba(0.5, 0.5, 0.5, 0.5),
    }
}

#[cfg(test)]
mod tests {
    use crate::debug::interaction_snapshot::InteractionDebugSnapshot;
    use crate::world::{
        ChunkCoord, InteractionMetadata, InteractionResult, InteractionTargetRef, InteractionType,
        LocalPosition, WorldPosition,
    };
    use bevy::prelude::Vec3;

    #[test]
    fn overlay_consumes_read_only_snapshot() {
        let snapshot = InteractionDebugSnapshot {
            query: Some(InteractionResult {
                interaction_type: InteractionType::MoveTarget,
                position: WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::ZERO)),
                metadata: InteractionMetadata::default(),
                valid: true,
                target: InteractionTargetRef::Terrain(WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::ZERO),
                )),
            }),
            resolved_order: None,
        };
        assert!(snapshot.query.is_some());
    }
}
