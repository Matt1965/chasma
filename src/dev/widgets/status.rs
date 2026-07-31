//! Compact status messages with severity (Slice 9).

use bevy::prelude::*;

use super::theme::{STATUS_ERROR, STATUS_INFO, STATUS_SUCCESS, STATUS_WARNING, small_text_font};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevStatusSeverity {
    Info,
    Success,
    Warning,
    Error,
}

impl DevStatusSeverity {
    pub fn color(self) -> Color {
        match self {
            Self::Info => STATUS_INFO,
            Self::Success => STATUS_SUCCESS,
            Self::Warning => STATUS_WARNING,
            Self::Error => STATUS_ERROR,
        }
    }

    /// Success messages expire; errors persist until replaced.
    pub fn default_ttl_frames(self) -> u32 {
        match self {
            Self::Success => 180,
            Self::Info => 120,
            Self::Warning | Self::Error => 0,
        }
    }
}

#[derive(Component, Debug)]
pub struct DevWidgetStatusLine {
    pub severity: DevStatusSeverity,
}

/// Apply severity color to a status text node.
pub fn sync_status_line_color(mut lines: Query<(&DevWidgetStatusLine, &mut TextColor)>) {
    for (line, mut color) in &mut lines {
        *color = TextColor(line.severity.color());
    }
}

/// Helper for catalog-style TTL status (re-export pattern).
pub fn status_text_color(severity: DevStatusSeverity) -> TextColor {
    TextColor(severity.color())
}

pub fn spawn_status_line(
    parent: &mut ChildSpawnerCommands<'_>,
    severity: DevStatusSeverity,
    initial: &str,
) {
    parent.spawn((
        DevWidgetStatusLine { severity },
        Text::new(initial),
        small_text_font(),
        TextColor(severity.color()),
        Node {
            min_height: Val::Px(14.0),
            ..default()
        },
    ));
}
