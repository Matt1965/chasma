//! Gizmo handle definitions and capability filtering (ADR-099).

use bevy::prelude::*;

use crate::world::authoring_transform::TransformCapabilities;

use crate::world::{BuildingCatalog, WorldData};

use super::tool::{DevTool, GizmoCoordinateSpace, SelectedWorldObject};

/// Interactive gizmo handle kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GizmoHandle {
    TranslateX,
    TranslateY,
    TranslateZ,
    TranslateXY,
    TranslateXZ,
    TranslateYZ,
    RotateX,
    RotateY,
    RotateZ,
    ScaleX,
    ScaleY,
    ScaleZ,
    ScaleUniform,
}

impl GizmoHandle {
    pub fn color(self) -> Color {
        match self {
            Self::TranslateX | Self::RotateX | Self::ScaleX => Color::srgba(0.95, 0.25, 0.25, 1.0),
            Self::TranslateY | Self::RotateY | Self::ScaleY => Color::srgba(0.30, 0.90, 0.35, 1.0),
            Self::TranslateZ | Self::RotateZ | Self::ScaleZ => Color::srgba(0.30, 0.45, 0.95, 1.0),
            Self::TranslateXY | Self::TranslateXZ | Self::TranslateYZ => {
                Color::srgba(0.85, 0.85, 0.85, 0.65)
            }
            Self::ScaleUniform => Color::srgba(0.95, 0.95, 0.95, 0.9),
        }
    }

    pub fn highlight_color(self) -> Color {
        match self {
            Self::TranslateX | Self::RotateX | Self::ScaleX => Color::srgba(1.0, 0.55, 0.55, 1.0),
            Self::TranslateY | Self::RotateY | Self::ScaleY => Color::srgba(0.55, 1.0, 0.55, 1.0),
            Self::TranslateZ | Self::RotateZ | Self::ScaleZ => Color::srgba(0.55, 0.65, 1.0, 1.0),
            Self::TranslateXY | Self::TranslateXZ | Self::TranslateYZ => {
                Color::srgba(1.0, 1.0, 1.0, 0.85)
            }
            Self::ScaleUniform => Color::srgba(1.0, 1.0, 1.0, 1.0),
        }
    }

    /// World-space unit axis for this handle (before local transform).
    pub fn axis(self) -> Option<Vec3> {
        match self {
            Self::TranslateX | Self::RotateX | Self::ScaleX => Some(Vec3::X),
            Self::TranslateY | Self::RotateY | Self::ScaleY => Some(Vec3::Y),
            Self::TranslateZ | Self::RotateZ | Self::ScaleZ => Some(Vec3::Z),
            _ => None,
        }
    }

    pub fn plane_normal(self) -> Option<Vec3> {
        match self {
            Self::TranslateXY => Some(Vec3::Z),
            Self::TranslateXZ => Some(Vec3::Y),
            Self::TranslateYZ => Some(Vec3::X),
            Self::RotateX => Some(Vec3::X),
            Self::RotateY => Some(Vec3::Y),
            Self::RotateZ => Some(Vec3::Z),
            _ => None,
        }
    }
}

/// Handles visible for the current tool, target, and capability policy.
pub fn active_handles(
    tool: DevTool,
    caps: TransformCapabilities,
    _space: GizmoCoordinateSpace,
) -> Vec<GizmoHandle> {
    let mut handles = Vec::new();
    match tool {
        DevTool::Translate => {
            if caps.translate_x {
                handles.push(GizmoHandle::TranslateX);
            }
            if caps.translate_y {
                handles.push(GizmoHandle::TranslateY);
            }
            if caps.translate_z {
                handles.push(GizmoHandle::TranslateZ);
            }
            if caps.translate_x && caps.translate_y {
                handles.push(GizmoHandle::TranslateXY);
            }
            if caps.translate_x && caps.translate_z {
                handles.push(GizmoHandle::TranslateXZ);
            }
            if caps.translate_y && caps.translate_z {
                handles.push(GizmoHandle::TranslateYZ);
            }
        }
        DevTool::Rotate => {
            if caps.rotate_x {
                handles.push(GizmoHandle::RotateX);
            }
            if caps.rotate_y {
                handles.push(GizmoHandle::RotateY);
            }
            if caps.rotate_z {
                handles.push(GizmoHandle::RotateZ);
            }
        }
        DevTool::Scale => {
            if caps.nonuniform_scale {
                handles.push(GizmoHandle::ScaleX);
                handles.push(GizmoHandle::ScaleY);
                handles.push(GizmoHandle::ScaleZ);
            }
            if caps.uniform_scale {
                handles.push(GizmoHandle::ScaleUniform);
            }
        }
        _ => {}
    }
    handles
}

/// Capability policy and commit permission for a selected object.
pub struct GizmoTargetPolicy {
    pub capabilities: TransformCapabilities,
    pub can_commit: bool,
    pub commit_blocked_reason: Option<&'static str>,
}

pub fn policy_for_target(
    target: SelectedWorldObject,
    building_catalog: &BuildingCatalog,
    world: &WorldData,
) -> GizmoTargetPolicy {
    match target {
        SelectedWorldObject::Doodad(_) => GizmoTargetPolicy {
            capabilities: TransformCapabilities::doodad(),
            can_commit: true,
            commit_blocked_reason: None,
        },
        SelectedWorldObject::Building(id) => {
            let capabilities = world
                .get_building(id)
                .and_then(|record| building_catalog.get(&record.definition_id))
                .map(|definition| definition.transform_safety_class.capabilities())
                .unwrap_or_else(TransformCapabilities::navigable_building);
            GizmoTargetPolicy {
                capabilities,
                can_commit: true,
                commit_blocked_reason: None,
            }
        }
        SelectedWorldObject::ItemPile(_) => GizmoTargetPolicy {
            capabilities: TransformCapabilities::world_item_pile(),
            can_commit: false,
            commit_blocked_reason: Some("Item pile transform commit not implemented"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doodad_translate_handles_include_planes() {
        let handles = active_handles(
            DevTool::Translate,
            TransformCapabilities::doodad(),
            GizmoCoordinateSpace::World,
        );
        assert!(handles.contains(&GizmoHandle::TranslateX));
        assert!(handles.contains(&GizmoHandle::TranslateXZ));
    }

    #[test]
    fn building_rotate_yaw_only() {
        let handles = active_handles(
            DevTool::Rotate,
            TransformCapabilities::navigable_building(),
            GizmoCoordinateSpace::World,
        );
        assert_eq!(handles, vec![GizmoHandle::RotateY]);
    }

    #[test]
    fn unit_has_no_handles() {
        let handles = active_handles(
            DevTool::Translate,
            TransformCapabilities::unit(),
            GizmoCoordinateSpace::World,
        );
        assert!(handles.is_empty());
    }
}
