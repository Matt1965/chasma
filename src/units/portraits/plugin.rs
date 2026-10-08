//! Portrait capture plugin wiring.

use bevy::mesh::InheritWeightSystems;
use bevy::prelude::*;

use crate::units::appearance_presentation::sync_unit_appearance_morphs;
use crate::units::equipment_presentation::finalize_skinned_equipment_overlays;
use crate::units::sync::UnitRuntimeSystems;

use super::cache::UnitPortraitCache;
use super::diagnostics::{
    PortraitDiagnosticTrace, sync_portrait_diagnostic_camera, sync_portrait_diagnostic_stage,
};
use super::equipment::sync_portrait_equipment_presentation;
use super::lifecycle_probe::{
    apply_portrait_lifecycle_probe, portrait_lifecycle_probe_input,
    record_portrait_lifecycle_trace, sync_portrait_lifecycle_camera_override,
    PortraitLifecycleProbe, PortraitLifecycleTrace,
};
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
            .init_resource::<PortraitLifecycleProbe>()
            .init_resource::<PortraitLifecycleTrace>()
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
                portrait_lifecycle_probe_input.in_set(UnitPortraitSystems),
            )
            .add_systems(
                Update,
                sync_portrait_lifecycle_camera_override
                    .after(sync_portrait_camera_active)
                    .in_set(UnitPortraitSystems),
            )
            .add_systems(
                PostUpdate,
                (
                    sync_portrait_equipment_presentation,
                    finalize_skinned_equipment_overlays,
                    update_portrait_actor_framing,
                    sync_portrait_capture_camera,
                    apply_portrait_lifecycle_probe,
                    propagate_portrait_render_layers,
                    drive_portrait_capture_pipeline,
                    record_portrait_lifecycle_trace,
                )
                    .chain()
                    .after(InheritWeightSystems)
                    .after(sync_unit_appearance_morphs)
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
