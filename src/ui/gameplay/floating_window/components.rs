//! Gameplay floating-window UI markers (BP5).

use bevy::prelude::*;

use super::id::FloatingGameplayWindowId;

/// Root shell for a draggable gameplay window.
#[derive(Component, Debug, Clone, Copy)]
pub struct FloatingGameplayWindowRoot {
    pub id: FloatingGameplayWindowId,
}

/// Title-bar drag handle — window moves only when dragging this region.
#[derive(Component, Debug, Clone, Copy)]
pub struct FloatingWindowTitleBarDragRegion {
    pub id: FloatingGameplayWindowId,
}
