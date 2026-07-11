//! Intent debug overlay — last dispatched intents and move targets.

use bevy::prelude::*;

use crate::client::ClientIntent;
use crate::client::commands::CommandTarget;
use crate::debug::settings::{DebugOverlayCategory, DebugOverlaySettings};
use crate::debug::trace::IntentDispatchHistory;
use crate::terrain::TerrainRenderAssets;
use crate::world::{WorldConfig, WorldPosition};

use super::helpers::{render_position, xz_to_render_y};

/// Draw gizmos for the most recent intent dispatch batch.
pub fn draw_intent_debug_overlay(
    mut gizmos: Gizmos,
    settings: Res<DebugOverlaySettings>,
    history: Res<IntentDispatchHistory>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.category_enabled(DebugOverlayCategory::Intent) {
        return;
    }

    let Some(report) = history.report.as_ref() else {
        return;
    };

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    for record in &report.records {
        match &record.intent {
            ClientIntent::MoveCommand { target } => {
                draw_move_target(&mut gizmos, *target, layout, vertical_scale);
            }
            ClientIntent::ContextualCommand { target } => {
                if let CommandTarget::Terrain { position } = target {
                    draw_move_target(&mut gizmos, *position, layout, vertical_scale);
                }
            }
            ClientIntent::SelectUnit { .. }
            | ClientIntent::ToggleUnitSelection { .. }
            | ClientIntent::BoxSelect { .. }
            | ClientIntent::BoxSelectAdd { .. } => {
                // Selection intents are visualized via selection overlay.
            }
            ClientIntent::ClearSelection | ClientIntent::ShiftModifier { .. } => {}
            ClientIntent::PaletteCommand { .. } => {}
        }
    }
}

fn draw_move_target(
    gizmos: &mut Gizmos,
    target: WorldPosition,
    layout: crate::world::ChunkLayout,
    vertical_scale: f32,
) {
    let center = xz_to_render_y(render_position(target, layout, vertical_scale), 0.25);
    gizmos.sphere(center, 0.35, Color::srgba(0.2, 0.85, 1.0, 0.75));
    gizmos.circle(
        Isometry3d::new(center, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        0.65,
        Color::srgba(0.2, 0.85, 1.0, 0.45),
    );
}
