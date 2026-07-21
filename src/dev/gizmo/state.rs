//! Client-local transform edit session state (ADR-099 DT3).

use bevy::prelude::*;

use crate::world::authoring_transform::{AuthoringScale, QuantizedOrientation};
use crate::world::{BuildingPlacement, DoodadPlacement, FixedScale, WorldPosition};

use super::handles::GizmoHandle;
use super::snap::TransformSnapSettings;
use super::tool::{DevTool, GizmoCoordinateSpace, SelectedWorldObject};

/// Preview placement for a doodad edit — mirrors authoritative [`DoodadPlacement`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DoodadPreviewPlacement {
    pub position: WorldPosition,
    pub orientation: QuantizedOrientation,
    pub scale: AuthoringScale,
}

impl DoodadPreviewPlacement {
    pub fn from_placement(placement: DoodadPlacement) -> Self {
        Self {
            position: placement.position,
            orientation: placement.orientation,
            scale: placement.scale,
        }
    }

    pub fn to_placement(self) -> DoodadPlacement {
        DoodadPlacement::new(self.position, self.orientation, self.scale)
    }

    pub fn rotation_quat(self) -> Quat {
        self.orientation.to_quat()
    }

    pub fn scale_vec3(self) -> Vec3 {
        self.scale.to_vec3()
    }
}

/// Convert authoritative building placement to gizmo preview (uniform scale only).
pub fn building_preview_from_placement(placement: BuildingPlacement) -> DoodadPreviewPlacement {
    DoodadPreviewPlacement {
        position: placement.position,
        orientation: QuantizedOrientation::from_quat(placement.rotation)
            .unwrap_or(QuantizedOrientation::IDENTITY),
        scale: AuthoringScale::Uniform(placement.uniform_scale),
    }
}

/// Convert gizmo preview back to building transform candidate fields.
pub fn building_uniform_scale_from_preview(preview: DoodadPreviewPlacement) -> FixedScale {
    match preview.scale {
        AuthoringScale::Uniform(scale) => scale,
        AuthoringScale::NonUniform { x, .. } => x,
    }
}

/// Client-local transform drag session. Not simulation truth.
#[derive(Resource, Debug, Clone, Default)]
pub struct TransformEditState {
    pub target: Option<SelectedWorldObject>,
    pub mode: DevTool,
    pub coordinate_space: GizmoCoordinateSpace,
    pub active_handle: Option<GizmoHandle>,
    pub hovered_handle: Option<GizmoHandle>,
    pub dragging: bool,
    pub drag_start_ray: Option<Ray3d>,
    pub drag_start_placement: Option<DoodadPreviewPlacement>,
    /// Camera-facing direction frozen at drag start (anchor → camera). Used for stable
    /// uniform scale drags; recomputing from the cursor ray each frame makes the scale
    /// plane spin and the drag math returns `None` or near-zero deltas.
    pub drag_scale_view_dir: Option<Vec3>,
    pub preview_placement: Option<DoodadPreviewPlacement>,
    pub snap: TransformSnapSettings,
    pub preview_valid: bool,
    pub last_error: String,
    pub axis_constraint: Option<GizmoAxisConstraint>,
}

/// Optional axis lock during drag (X/Y/Z keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoAxisConstraint {
    X,
    Y,
    Z,
}

impl TransformEditState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn is_transform_session_active(&self) -> bool {
        self.target.is_some() && self.mode.is_transform()
    }

    pub fn begin_drag(
        &mut self,
        handle: GizmoHandle,
        ray: Ray3d,
        start: DoodadPreviewPlacement,
        scale_view_dir: Vec3,
    ) {
        self.active_handle = Some(handle);
        self.dragging = true;
        self.drag_start_ray = Some(ray);
        self.drag_start_placement = Some(start);
        self.drag_scale_view_dir = if scale_view_dir.length_squared() > 1e-6 {
            Some(scale_view_dir.normalize())
        } else {
            None
        };
        self.preview_placement = Some(start);
        self.preview_valid = true;
        self.last_error.clear();
    }

    pub fn cancel_drag(&mut self) {
        if let Some(start) = self.drag_start_placement {
            self.preview_placement = Some(start);
        }
        self.active_handle = None;
        self.dragging = false;
        self.drag_start_ray = None;
        self.drag_start_placement = None;
        self.drag_scale_view_dir = None;
        self.axis_constraint = None;
        self.preview_valid = true;
        self.last_error.clear();
    }

    pub fn end_drag(&mut self) {
        self.active_handle = None;
        self.dragging = false;
        self.drag_start_ray = None;
        self.drag_start_placement = None;
        self.drag_scale_view_dir = None;
        self.axis_constraint = None;
    }

    pub fn full_cancel(&mut self) {
        self.clear();
    }

    pub fn sync_target_from_selection(
        &mut self,
        target: Option<SelectedWorldObject>,
        tool: DevTool,
        authoritative: Option<DoodadPreviewPlacement>,
    ) {
        if self.dragging {
            return;
        }
        if self.target != target {
            self.target = target;
            self.preview_placement = authoritative;
            self.last_error.clear();
        }
        if tool.is_transform() {
            self.mode = tool;
        }
    }
}
