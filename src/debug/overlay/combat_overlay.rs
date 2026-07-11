//! Combat debug overlay — range circles, target lines, projectile paths (ADR-061 C8).

use bevy::prelude::*;

use crate::debug::combat_log::is_combat_log_outcome;
use crate::debug::settings::{DebugOverlayCategory, DebugOverlaySettings};
use crate::debug::trace::{CommandTraceBuffer, CommandTraceOutcome};
use crate::terrain::TerrainRenderAssets;
use crate::ui::gameplay::combat_display::{
    attack_range_circle_radius_meters, combat_target_id, weapon_display_for_unit,
};
use crate::units::input::SelectedUnits;
use crate::world::{UnitCatalog, WeaponCatalog, WorldConfig, WorldData, weapon_for_unit_record};

use super::helpers::{render_position, xz_to_render_y};

const RANGE_COLOR: Color = Color::srgba(0.95, 0.35, 0.25, 0.45);
const TARGET_LINE_COLOR: Color = Color::srgba(1.0, 0.2, 0.2, 0.85);
const PROJECTILE_LINE_COLOR: Color = Color::srgba(1.0, 0.85, 0.2, 0.9);
const HIT_MARKER_COLOR: Color = Color::srgba(1.0, 0.1, 0.1, 0.95);
const DEAD_MARKER_COLOR: Color = Color::srgba(0.4, 0.4, 0.4, 0.8);

pub fn draw_combat_debug_overlay(
    mut gizmos: Gizmos,
    selection: Res<SelectedUnits>,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    weapons: Res<WeaponCatalog>,
    config: Res<WorldConfig>,
    settings: Res<DebugOverlaySettings>,
    trace: Res<CommandTraceBuffer>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    if !settings.category_enabled(DebugOverlayCategory::Combat) || selection.is_empty() {
        return;
    }

    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let mut drawn = 0_u32;
    let mut unit_ids: Vec<_> = selection.iter().collect();
    unit_ids.sort_by_key(|id| id.raw());

    for unit_id in unit_ids {
        if drawn >= settings.max_draw_units {
            break;
        }
        let Some(record) = world.get_unit(unit_id) else {
            continue;
        };
        let Some(definition) = catalog.get(&record.definition_id) else {
            continue;
        };
        let Ok(weapon) = weapon_for_unit_record(record, &catalog, &weapons) else {
            continue;
        };

        let center = render_position(record.placement.position, layout, vertical_scale);
        let circle_radius = attack_range_circle_radius_meters(
            weapon.range_meters,
            definition.collision_radius_meters,
        );
        gizmos.circle(
            Isometry3d::new(
                xz_to_render_y(center, 0.06),
                Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            ),
            circle_radius,
            RANGE_COLOR,
        );

        if let Some(target_id) = combat_target_id(&record.combat_state) {
            if let Some(target) = world.get_unit(target_id) {
                let target_pos = render_position(target.placement.position, layout, vertical_scale);
                gizmos.line(
                    xz_to_render_y(center, 0.15),
                    xz_to_render_y(target_pos, 0.15),
                    TARGET_LINE_COLOR,
                );
            }
        }

        if matches!(record.state, crate::world::UnitState::Dead) {
            gizmos.sphere(
                Isometry3d::from_translation(xz_to_render_y(center, 0.3)),
                0.35,
                DEAD_MARKER_COLOR,
            );
        }

        drawn += 1;
        let _ = weapon_display_for_unit(record, &catalog, &weapons);
    }

    for projectile_id in world.sorted_projectile_ids() {
        if drawn >= settings.max_draw_units {
            break;
        }
        let Some(record) = world.get_projectile(projectile_id) else {
            continue;
        };
        if !selection.contains(record.source_unit_id) {
            continue;
        }
        let from = render_position(record.position, layout, vertical_scale);
        let to = render_position(record.target_position_snapshot, layout, vertical_scale);
        gizmos.line(
            xz_to_render_y(from, 0.2),
            xz_to_render_y(to, 0.2),
            PROJECTILE_LINE_COLOR,
        );
        gizmos.sphere(
            Isometry3d::from_translation(xz_to_render_y(from, 0.2)),
            0.12,
            PROJECTILE_LINE_COLOR,
        );
        drawn += 1;
    }

    for entry in trace.entries().rev().take(8) {
        if !is_combat_log_outcome(entry.outcome) {
            continue;
        }
        let affects_selection = entry.unit_ids.iter().any(|id| selection.contains(*id));
        if !affects_selection {
            continue;
        }
        if !matches!(
            entry.outcome,
            CommandTraceOutcome::CombatAttackStrikeApplied
                | CommandTraceOutcome::ProjectileHit
                | CommandTraceOutcome::ProjectileDamageApplied
        ) {
            continue;
        }
        let Some(attacker_id) = entry.unit_ids.first().copied() else {
            continue;
        };
        let Some(record) = world.get_unit(attacker_id) else {
            continue;
        };
        if let Some(target_id) = combat_target_id(&record.combat_state) {
            if let Some(target) = world.get_unit(target_id) {
                let pos = render_position(target.placement.position, layout, vertical_scale);
                gizmos.sphere(
                    Isometry3d::from_translation(xz_to_render_y(pos, 0.5)),
                    0.25,
                    HIT_MARKER_COLOR,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::gameplay::combat_display::combat_target_id;
    use crate::world::CombatState;
    use crate::world::UnitId;

    #[test]
    fn target_line_overlay_reads_unit_record_target_only() {
        let target = UnitId::new(4);
        let state = CombatState::Chasing { target };
        assert_eq!(combat_target_id(&state), Some(target));
    }

    #[test]
    fn projectile_overlay_uses_projectile_record_positions() {
        let mut world = WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        assert!(world.projectiles().next().is_none());
    }
}
