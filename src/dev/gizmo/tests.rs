//! Gizmo tool and state tests (ADR-099).

#[cfg(test)]
mod tool_state_tests {
    use bevy::prelude::*;

    use crate::dev::gizmo::handles::{GizmoHandle, active_handles};
    use crate::dev::gizmo::state::{DoodadPreviewPlacement, TransformEditState};
    use crate::dev::gizmo::tool::{DevTool, GizmoCoordinateSpace};
    use crate::world::{
        AuthoringScale, ChunkCoord, DoodadPlacement, LocalPosition, QuantizedOrientation,
        TransformCapabilities, WorldPosition,
    };

    fn placement() -> DoodadPreviewPlacement {
        DoodadPreviewPlacement::from_placement(DoodadPlacement::identity_at(WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::ZERO),
        )))
    }

    #[test]
    fn cancel_drag_restores_start_placement() {
        let mut edit = TransformEditState::default();
        let start = placement();
        edit.begin_drag(
            GizmoHandle::TranslateX,
            Ray3d::new(Vec3::ZERO, Dir3::X),
            start,
        );
        edit.preview_placement = Some(DoodadPreviewPlacement {
            position: WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(5.0, 0.0, 0.0)),
            ),
            orientation: QuantizedOrientation::IDENTITY,
            scale: AuthoringScale::uniform_one(),
        });
        edit.cancel_drag();
        assert_eq!(edit.preview_placement, Some(start));
        assert!(!edit.dragging);
    }

    #[test]
    fn dev_tool_transform_modes() {
        assert!(DevTool::Translate.is_transform());
        assert!(!DevTool::Select.is_transform());
        assert!(!DevTool::Place.is_transform());
    }

    #[test]
    fn scale_handles_for_doodad() {
        let handles = active_handles(
            DevTool::Scale,
            TransformCapabilities::doodad(),
            GizmoCoordinateSpace::Local,
        );
        assert!(handles.contains(&GizmoHandle::ScaleUniform));
        assert!(handles.contains(&GizmoHandle::ScaleX));
    }
}
