//! Tooltip target components and content model (Slice 9).

use bevy::prelude::*;

/// Rich tooltip content (rendered to plain text for the shared popup).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DevTooltipContent {
    pub title: Option<String>,
    pub body: String,
    pub units: Option<String>,
    pub scope: Option<String>,
    pub shortcut: Option<String>,
    pub disabled_reason: Option<String>,
}

impl DevTooltipContent {
    pub fn new(body: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            ..Default::default()
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn units(mut self, units: impl Into<String>) -> Self {
        self.units = Some(units.into());
        self
    }

    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        self.disabled_reason = Some(reason.into());
        self
    }

    pub fn format(&self) -> String {
        let mut lines = Vec::new();
        if let Some(title) = &self.title {
            lines.push(title.clone());
        }
        if !self.body.is_empty() {
            lines.push(self.body.clone());
        }
        if let Some(units) = &self.units {
            lines.push(format!("Units: {units}"));
        }
        if let Some(scope) = &self.scope {
            lines.push(format!("Scope: {scope}"));
        }
        if let Some(shortcut) = &self.shortcut {
            lines.push(format!("Shortcut: {shortcut}"));
        }
        if let Some(reason) = &self.disabled_reason {
            lines.push(format!("Disabled: {reason}"));
        }
        lines.join("\n")
    }
}

/// Attach to any dev UI control that should show a hover tooltip.
#[derive(Component, Debug, Clone)]
pub struct DevTooltipTarget {
    pub content: DevTooltipContent,
}

impl DevTooltipTarget {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            content: DevTooltipContent::new(text),
        }
    }

    pub fn from_content(content: DevTooltipContent) -> Self {
        Self { content }
    }

    pub fn text(&self) -> String {
        self.content.format()
    }
}

/// Hover zone for disabled controls that do not receive Button interaction.
#[derive(Component, Debug, Clone)]
pub struct DevTooltipHoverZone {
    pub content: DevTooltipContent,
}

impl DevTooltipHoverZone {
    pub fn from_content(content: DevTooltipContent) -> Self {
        Self { content }
    }
}
