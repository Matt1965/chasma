//! Dev-only preview unit placement near the initial camera focus (ADR-028).
//!
//! Spawns instances for Excel catalog rows that include a `File Path` (render key).
//! Starter-catalog fallback is used only when the workbook import fails entirely.

use bevy::prelude::*;

use crate::camera::CameraSettings;
use crate::terrain::residency::ChunkResidencyTracker;
use crate::world::{
    create_unit, ground_unit_to_terrain, ChunkId, UnitCatalog, UnitDefinition,
    UnitGroundingError, UnitSource, WorldConfig, WorldData, WorldPosition,
};

/// Dev preview unit count (grid near camera focus).
pub const DEV_PREVIEW_UNIT_COUNT: usize = 100;

/// XZ spacing between grid slots (meters).
const DEV_PREVIEW_GRID_SPACING_METERS: f32 = 4.0;

/// East of camera focus so spawns land on interior terrain rather than chunk seams.
const DEV_PREVIEW_BASE_OFFSET_X: f32 = 40.0;

/// Grid offsets from [`CameraSettings::initial_focus`].
fn dev_preview_spawn_offsets(count: usize) -> Vec<(f32, f32)> {
    let cols = (count as f32).sqrt().ceil() as usize;
    (0..count)
        .map(|index| {
            let row = index / cols;
            let col = index % cols;
            let offset_x =
                DEV_PREVIEW_BASE_OFFSET_X + col as f32 * DEV_PREVIEW_GRID_SPACING_METERS;
            let offset_z = (row as f32 - (cols.saturating_sub(1)) as f32 * 0.5)
                * DEV_PREVIEW_GRID_SPACING_METERS;
            (offset_x, offset_z)
        })
        .collect()
}

#[derive(Resource, Default, Debug)]
pub struct DevPreviewUnitSpawnLedger {
    completed: bool,
    warned_no_renderable_definitions: bool,
}

/// Insert preview units into [`WorldData`] when their chunk terrain is resident.
pub fn spawn_dev_preview_units(
    camera: Res<CameraSettings>,
    config: Res<WorldConfig>,
    catalog: Res<UnitCatalog>,
    residency: Res<ChunkResidencyTracker>,
    mut world: ResMut<WorldData>,
    mut ledger: ResMut<DevPreviewUnitSpawnLedger>,
) {
    if ledger.completed {
        return;
    }

    let renderable: Vec<&UnitDefinition> = catalog
        .enabled_definitions()
        .filter(|def| def.render_key.0.is_some())
        .collect();

    if renderable.is_empty() {
        if !ledger.warned_no_renderable_definitions {
            warn!(
                "dev preview unit spawn skipped: no Units sheet rows with `File Path` \
                 (add column `File Path` and set e.g. `\\units\\robot.glb` on the robot row)"
            );
            ledger.warned_no_renderable_definitions = true;
        }
        return;
    }

    let layout = config.chunk_layout();
    let focus = camera.initial_focus;
    let spawn_plan = build_spawn_plan(&renderable);

    for (_, offset_x, offset_z) in &spawn_plan {
        let global = Vec3::new(focus.x + offset_x, 0.0, focus.z + offset_z);
        let position = WorldPosition::from_global(global, layout);
        let chunk = ChunkId::new(position.chunk);
        if !residency.is_resident(chunk) {
            return;
        }
    }

    let mut spawned = 0u32;
    for (definition, offset_x, offset_z) in spawn_plan {
        let global = Vec3::new(focus.x + offset_x, 0.0, focus.z + offset_z);
        let position = WorldPosition::from_global(global, layout);

        let record = match create_unit(
            &catalog,
            &mut world,
            &definition.id,
            position,
            UnitSource::Authored,
        ) {
            Ok(record) => record,
            Err(err) => {
                warn!(
                    "dev preview unit spawn failed for `{}`: {err:?}",
                    definition.id.as_str()
                );
                return;
            }
        };

        if let Err(err) = ground_unit_to_terrain(&mut world, record.id) {
            if err == UnitGroundingError::TerrainUnavailable {
                return;
            }
            warn!(
                "dev preview unit grounding failed for `{}`: {err:?}",
                definition.id.as_str()
            );
            return;
        }
        spawned += 1;
    }

    info!(
        "dev preview spawned {spawned} unit(s) near camera focus ({:.0}, {:.0})",
        focus.x, focus.z
    );
    ledger.completed = true;
}

fn build_spawn_plan<'a>(
    renderable: &[&'a UnitDefinition],
) -> Vec<(&'a UnitDefinition, f32, f32)> {
    let offsets = dev_preview_spawn_offsets(DEV_PREVIEW_UNIT_COUNT);
    if renderable.len() == 1 {
        let definition = renderable[0];
        return offsets
            .into_iter()
            .map(|(offset_x, offset_z)| (definition, offset_x, offset_z))
            .collect();
    }

    offsets
        .into_iter()
        .enumerate()
        .map(|(index, (offset_x, offset_z))| {
            let definition = renderable[index % renderable.len()];
            (definition, offset_x, offset_z)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{UnitDefinitionId, UnitRenderKey};
    use crate::world::UnitDefinition;

    fn stub_definition(id: &str, key: Option<&str>) -> UnitDefinition {
        UnitDefinition::new(
            UnitDefinitionId::new(id),
            id,
            "Test",
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1,
            1.0,
            "Common",
            4.0,
            0.5,
            40.0,
            true,
            match key {
                Some(key) => UnitRenderKey::reserved(key),
                None => UnitRenderKey::unset(),
            },
        )
    }

    #[test]
    fn single_renderable_spawns_hundred_copies() {
        let robot = stub_definition("robot", Some("robot"));
        let plan = build_spawn_plan(&[&robot]);
        assert_eq!(plan.len(), DEV_PREVIEW_UNIT_COUNT);
        assert!(plan.iter().all(|(def, _, _)| def.id.as_str() == "robot"));
    }

    #[test]
    fn multiple_renderables_fill_grid_with_round_robin() {
        let robot = stub_definition("robot", Some("robot"));
        let wolf = stub_definition("wolf", Some("wolf"));
        let plan = build_spawn_plan(&[&robot, &wolf]);
        assert_eq!(plan.len(), DEV_PREVIEW_UNIT_COUNT);
        assert_eq!(plan[0].0.id.as_str(), "robot");
        assert_eq!(plan[1].0.id.as_str(), "wolf");
    }
}
