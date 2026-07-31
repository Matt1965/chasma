//! Read-only status badges (Slice 9).

use bevy::prelude::*;

use crate::dev::tooltip::DevTooltipContent;
use crate::dev::tooltip::DevTooltipTarget;

use super::theme::{BADGE_BG, BADGE_TEXT, small_text_font};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevBadgeKind {
    AssetDefault,
    InstanceOverride,
    Generated,
    Valid,
    Warning,
    Error,
    Dirty,
    ReadOnly,
    DevOnly,
}

impl DevBadgeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::AssetDefault => "Asset Default",
            Self::InstanceOverride => "Instance Override",
            Self::Generated => "Generated",
            Self::Valid => "Valid",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Dirty => "Dirty",
            Self::ReadOnly => "Read Only",
            Self::DevOnly => "Dev Only",
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DevWidgetBadge {
    pub kind: DevBadgeKind,
}

pub fn spawn_badge(
    parent: &mut ChildSpawnerCommands<'_>,
    kind: DevBadgeKind,
    tooltip: DevTooltipContent,
) {
    parent.spawn((
        DevWidgetBadge { kind },
        DevTooltipTarget::from_content(tooltip),
        Node {
            padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(BADGE_BG),
        Text::new(kind.label()),
        small_text_font(),
        TextColor(BADGE_TEXT),
    ));
}
