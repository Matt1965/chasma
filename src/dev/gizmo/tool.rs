//! Dev transform tool and selection types (ADR-099 DT3).

use bevy::prelude::*;

use crate::world::{BuildingId, DoodadId, ItemPileId};

/// Active dev authoring tool (client-local, ADR-099).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Reflect)]
pub enum DevTool {
    #[default]
    Select,
    Place,
    Translate,
    Rotate,
    Scale,
}

impl DevTool {
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Place => "Place",
            Self::Translate => "Translate",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
        }
    }

    pub fn is_transform(self) -> bool {
        matches!(self, Self::Translate | Self::Rotate | Self::Scale)
    }
}

/// Client-local active dev tool.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub struct DevToolState {
    pub active_tool: DevTool,
}

/// Generalized dev-editable world object reference (ADR-099).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectedWorldObject {
    Doodad(DoodadId),
    Building(BuildingId),
    ItemPile(ItemPileId),
}

/// Coordinate space for translation/rotation gizmo handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Reflect)]
pub enum GizmoCoordinateSpace {
    #[default]
    World,
    Local,
}

impl GizmoCoordinateSpace {
    pub fn toggle(self) -> Self {
        match self {
            Self::World => Self::Local,
            Self::Local => Self::World,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::World => "World",
            Self::Local => "Local",
        }
    }
}
