//! Catalog UI marker components (Slice 4).

use bevy::prelude::*;

use super::placement_controls::PlacementControlField;

use crate::dev::dev_mode::DevTab;

#[derive(Component, Debug)]

pub(crate) struct DevCatalogStatusText;

#[derive(Component, Debug)]

pub(crate) struct DevPlacementActiveBanner;

#[derive(Component, Debug)]

pub(crate) struct DevContextualPlacementButton {
    pub action: DevContextualPlacementAction,

    pub field: PlacementControlField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub(crate) enum DevContextualPlacementAction {
    CycleBrush,

    CountUp,

    CountDown,

    SpacingUp,

    SpacingDown,

    RadiusUp,

    RadiusDown,

    GridColsUp,

    GridColsDown,

    GridRowsUp,

    GridRowsDown,

    ToggleTerrainSnap,

    TogglePreview,

    CycleSpawnTeam,

    RotationUp,

    RotationDown,

    ScaleUp,

    ScaleDown,

    CancelPlacement,
}

#[derive(Component, Debug)]

pub(crate) struct DevContextualPlacementSection;

#[derive(Component, Debug)]

pub(crate) struct DevContextualPlacementTitle;

#[derive(Component, Debug)]

pub(crate) struct DevTabChrome {
    pub tab: DevTab,
}
