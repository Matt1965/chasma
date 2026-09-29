//! Ghost preview for dev placement (ADR-044).
//!
//! [`DevPlacementPreview`] / [`PreviewPoint`] remain the authoritative placement plan.
//! Translucent model ghosts are synced separately in `preview_model.rs`.

use bevy::prelude::*;

use crate::doodads::DoodadsRuntimeSettings;
use crate::terrain::TerrainRenderAssets;
use crate::world::{
    BuildingCatalog, DoodadCatalog, FootprintCatalog, UnitCatalog, WorldConfig, WorldData,
    WorldPosition,
};

use super::super::dev_mode::{DefinitionId, DevModeState, DevTab};
use super::batch_spawn::{BatchSpawnRequest, BatchSpawnScratch, plan_batch_spawn};
use super::placement_rules::{PlacementValidateContext, PlacementValidation, validate_placement};

/// One preview marker (simulation-independent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewPoint {
    pub position: WorldPosition,
    pub valid: bool,
}

/// Client-local preview state updated from cursor / brush settings.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct DevPlacementPreview {
    pub active: bool,
    pub points: Vec<PreviewPoint>,
}

impl DevPlacementPreview {
    pub fn clear(&mut self) {
        self.active = false;
        self.points.clear();
    }
}

/// Reusable scratch for preview planning.
#[derive(Resource, Debug, Default)]
pub struct DevPlacementPreviewScratch {
    batch: BatchSpawnScratch,
}

/// Last terrain anchor under cursor for preview (client-local).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct DevPreviewAnchor {
    pub position: WorldPosition,
}

impl Default for DevPreviewAnchor {
    fn default() -> Self {
        Self {
            position: WorldPosition::new(
                crate::world::ChunkCoord::new(0, 0),
                crate::world::LocalPosition::new(Vec3::ZERO),
            ),
        }
    }
}

/// Update preview from dev state + cursor anchor.
pub fn update_dev_placement_preview(
    dev_state: Res<DevModeState>,
    mut preview: ResMut<DevPlacementPreview>,
    mut scratch: ResMut<DevPlacementPreviewScratch>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    unit_catalog: Res<UnitCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    building_catalog: Res<BuildingCatalog>,
    footprint_catalog: Res<FootprintCatalog>,
    runtime: Option<Res<DoodadsRuntimeSettings>>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    anchor: Option<Res<DevPreviewAnchor>>,
) {
    preview.clear();
    if !dev_state.enabled || !dev_state.show_preview {
        return;
    }
    let Some(definition) = dev_state.selected_definition.as_ref() else {
        return;
    };
    let Some(anchor_res) = anchor else {
        return;
    };
    let anchor_pos = anchor_res.position;

    let request = BatchSpawnRequest {
        definition: definition.clone(),
        brush: dev_state.brush,
        anchor: anchor_pos,
        line_direction: dev_state.last_line_direction,
        terrain_conforming: dev_state.terrain_conforming,
        rules: dev_state.placement_rules,
        world_seed: runtime
            .as_ref()
            .map(|r| r.world_seed)
            .unwrap_or(crate::doodads::DEFAULT_DOODAD_WORLD_SEED),
        layout: config.chunk_layout(),
        spawn_affiliation: dev_state.spawn_affiliation,
        unit_spawn_ownership: dev_state.unit_spawn_ownership(),
        unit_spawn_faction_id: dev_state.spawn_faction_id.clone(),
        placement_yaw_deg: dev_state.placement_yaw_deg,
        placement_uniform_scale: dev_state.placement_uniform_scale,
        terrain_vertical_scale: render_assets
            .as_ref()
            .map(|assets| assets.vertical_scale)
            .unwrap_or(1.0),
        unit_archetype: dev_state.selected_unit_archetype_for_spawn().cloned(),
        building_archetype: dev_state.selected_building_archetype_for_spawn().cloned(),
    };

    let ctx = PlacementValidateContext {
        world: &world,
        unit_catalog: &unit_catalog,
        doodad_catalog: &doodad_catalog,
        building_catalog: &building_catalog,
        footprint_catalog: &footprint_catalog,
        definition,
        rules: &dev_state.placement_rules,
    };

    if matches!(definition, DefinitionId::Item(_)) {
        if dev_state.active_tab == DevTab::Items
            && (dev_state.dev_held_item_id().is_some() || dev_state.inventory.pile_placement_armed)
        {
            preview.points.push(PreviewPoint {
                position: anchor_pos,
                valid: true,
            });
        }
    } else {
        generate_preview_points(
            &request,
            definition.id_str(),
            &world,
            &unit_catalog,
            &doodad_catalog,
            &building_catalog,
            &ctx,
            &mut scratch.batch,
            &mut preview.points,
        );
    }
    preview.active = !preview.points.is_empty();
}

fn generate_preview_points(
    request: &BatchSpawnRequest,
    definition_key: &str,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    ctx: &PlacementValidateContext<'_>,
    scratch: &mut BatchSpawnScratch,
    out: &mut Vec<PreviewPoint>,
) {
    let _ = plan_batch_spawn(
        request,
        definition_key,
        world,
        unit_catalog,
        doodad_catalog,
        building_catalog,
        ctx.footprint_catalog,
        scratch,
    );
    let mut accepted: Vec<WorldPosition> = Vec::new();
    out.clear();
    out.reserve(scratch.candidate_positions().len());
    for &candidate in scratch.candidate_positions() {
        let valid = match validate_placement(ctx, candidate, &accepted) {
            PlacementValidation::Accepted(position) => {
                accepted.push(position);
                true
            }
            PlacementValidation::Rejected(_) => false,
        };
        let display = if valid {
            *accepted.last().unwrap()
        } else {
            candidate
        };
        out.push(PreviewPoint {
            position: display,
            valid,
        });
    }
}

