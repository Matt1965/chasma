//! Interaction classification debug overlay (ADR-042 U6 + ADR-039 U-UI3).

use bevy::prelude::*;

use crate::client::ClientIntent;
use crate::client::commands::CommandTarget;
use crate::debug::settings::{DebugOverlayCategory, DebugOverlaySettings};
use crate::debug::trace::IntentDispatchHistory;
use crate::terrain::TerrainRenderAssets;
use crate::world::{
    interaction_plan_to_unit_order, query_world_interaction, resolve_interaction_to_order,
    DoodadCatalog, InteractionDebugSnapshot, InteractionQueryContext, InteractionType,
    UnitCatalog, WorldConfig, WorldData, WorldPosition,
};

use super::helpers::{render_position, xz_to_render_y};

/// Sample the last dispatched click against world interaction and draw classification.
pub fn draw_interaction_debug_overlay(
    mut gizmos: Gizmos,
    settings: Res<DebugOverlaySettings>,
    history: Res<IntentDispatchHistory>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    doodad_catalog: Res<DoodadCatalog>,
    unit_catalog: Res<UnitCatalog>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut snapshot: ResMut<InteractionDebugSnapshot>,
) {
    if !settings.category_enabled(DebugOverlayCategory::Interaction) {
        return;
    }

    let Some(position) = last_command_target_position(&history, &world) else {
        return;
    };

    let ctx = InteractionQueryContext::new(&world, &doodad_catalog, &unit_catalog);
    let Some(interaction) = query_world_interaction(&ctx, position) else {
        snapshot.clear();
        return;
    };

    let plan = resolve_interaction_to_order(&interaction);
    let order = interaction_plan_to_unit_order(plan);
    snapshot.record_query_and_order(interaction.clone(), order);

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

fn last_command_target_position(
    history: &IntentDispatchHistory,
    world: &WorldData,
) -> Option<WorldPosition> {
    let report = history.report.as_ref()?;
    for record in report.records.iter().rev() {
        match &record.intent {
            ClientIntent::ContextualCommand {
                target: CommandTarget::Terrain { position },
            }
            | ClientIntent::MoveCommand { target: position } => return Some(*position),
            ClientIntent::ContextualCommand {
                target: CommandTarget::Unit { unit_id },
            } => {
                return world
                    .get_unit(*unit_id)
                    .map(|record| record.placement.position);
            }
            _ => {}
        }
    }
    None
}

fn interaction_type_color(kind: InteractionType) -> Color {
    match kind {
        InteractionType::MoveTarget => Color::srgba(0.2, 0.85, 0.35, 0.85),
        InteractionType::ResourceNode => Color::srgba(0.95, 0.75, 0.15, 0.9),
        InteractionType::InteractableObject => Color::srgba(0.35, 0.65, 1.0, 0.85),
        InteractionType::BlockedArea => Color::srgba(0.95, 0.25, 0.2, 0.85),
        InteractionType::TerrainPoint => Color::srgba(0.7, 0.7, 0.7, 0.8),
        InteractionType::None => Color::srgba(0.5, 0.5, 0.5, 0.5),
    }
}
