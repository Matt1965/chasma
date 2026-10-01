//! Portrait capture plugin wiring.

use bevy::mesh::InheritWeightSystems;
use bevy::prelude::*;

use crate::units::equipment_presentation::finalize_skinned_equipment_overlays;
use crate::units::sync::UnitRuntimeSystems;

use super::cache::UnitPortraitCache;
use super::diagnostics::{
    PortraitDiagnosticTrace, sync_portrait_diagnostic_camera, sync_portrait_diagnostic_stage,
};
use super::equipment::sync_portrait_equipment_presentation;
use super::pipeline::{
    UnitPortraitCaptureState, UnitPortraitUiDemand, drive_portrait_capture_pipeline,
    maintain_portrait_cache_requests, sync_portrait_capture_camera, update_portrait_actor_framing,
};
use super::studio::{
    propagate_portrait_render_layers, setup_unit_portrait_studio, sync_portrait_camera_active,
};

/// Systems that capture and cache unit portraits for gameplay HUD.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct UnitPortraitSystems;

pub struct UnitPortraitPlugin;

impl Plugin for UnitPortraitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UnitPortraitCache>()
            .init_resource::<UnitPortraitCaptureState>()
            .init_resource::<UnitPortraitUiDemand>()
            .init_resource::<PortraitDiagnosticTrace>()
            .init_resource::<super::equipment::UnitPortraitEquipmentIndex>()
            .add_systems(Startup, setup_unit_portrait_studio)
            .add_systems(
                Update,
                maintain_portrait_cache_requests.in_set(UnitPortraitSystems),
            )
            .add_systems(
                Update,
                propagate_portrait_render_layers.in_set(UnitPortraitSystems),
            )
            .add_systems(
                Update,
                sync_portrait_diagnostic_stage.in_set(UnitPortraitSystems),
            )
            .add_systems(
                Update,
                sync_portrait_diagnostic_camera.in_set(UnitPortraitSystems),
            )
            .add_systems(
                Update,
                sync_portrait_camera_active.in_set(UnitPortraitSystems),
            )
            .add_systems(
                Update,
                sync_portrait_capture_camera.in_set(UnitPortraitSystems),
            )
            .add_systems(
                Update,
                update_portrait_actor_framing.in_set(UnitPortraitSystems),
            )
            .add_systems(
                PostUpdate,
                (
                    sync_portrait_equipment_presentation,
                    finalize_skinned_equipment_overlays,
                )
                    .chain()
                    .after(InheritWeightSystems)
                    .in_set(UnitPortraitSystems),
            )
            .add_systems(
                PostUpdate,
                (
                    propagate_portrait_render_layers,
                    drive_portrait_capture_pipeline,
                )
                    .chain()
                    .in_set(UnitPortraitSystems),
            );
    }
}

/// Portrait cache requests should follow gameplay unit presentation sync.
pub fn configure_portrait_system_sets(app: &mut App) {
    app.configure_sets(
        Update,
        UnitPortraitSystems.after(UnitRuntimeSystems),
    );
}
