//! Dev-window UI markers (Slice 3).

use bevy::prelude::*;

use super::id::DevWindowId;

/// Marker for any dev-window UI node (hover / input blocking).

#[derive(Component, Debug, Clone, Copy, Default)]

pub struct DevWindowUi;

/// Root shell for one dev window.

#[derive(Component, Debug, Clone, Copy)]

pub struct DevWindowRoot {
    pub id: DevWindowId,
}

/// Draggable title-bar region — drag starts only here.

#[derive(Component, Debug, Clone, Copy)]

pub struct DevWindowTitleBarDragRegion {
    pub id: DevWindowId,
}

/// Window body container (hidden when collapsed).

#[derive(Component, Debug, Clone, Copy)]

pub struct DevWindowBody {
    pub id: DevWindowId,
}

/// Close button — hides the window.

#[derive(Component, Debug, Clone, Copy)]

pub struct DevWindowCloseButton {
    pub id: DevWindowId,
}

/// Collapse/expand toggle.

#[derive(Component, Debug, Clone, Copy)]

pub struct DevWindowCollapseButton {
    pub id: DevWindowId,
}

/// Label child of the collapse title-bar button (`-` / `+`).

#[derive(Component, Debug, Clone, Copy)]

pub struct DevWindowCollapseButtonLabel {
    pub id: DevWindowId,
}

/// Persistent workspace launcher container.

#[derive(Component, Debug, Clone, Copy, Default)]

pub struct DevWorkspaceLauncher;

/// Which launcher row a toggle or button row belongs to.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum DevLauncherGroup {
    #[default]
    Windows,

    Advanced,
}

/// Clickable header that expands/collapses one launcher row.

#[derive(Component, Debug, Clone, Copy)]

pub struct DevWorkspaceLauncherToggle {
    pub group: DevLauncherGroup,
}

/// Container for per-window launcher buttons (hidden when its row is collapsed).

#[derive(Component, Debug, Clone, Copy)]

pub struct DevWorkspaceLauncherButtons {
    pub group: DevLauncherGroup,
}

/// Launcher button to show a specific window.

#[derive(Component, Debug, Clone, Copy)]

pub struct DevWorkspaceLauncherButton {
    pub window: DevWindowId,
}
