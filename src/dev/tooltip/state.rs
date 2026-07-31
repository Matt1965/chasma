//! Client-local tooltip presentation state.

use bevy::prelude::*;

/// Active tooltip content and screen position (presentation only).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct DevTooltipState {
    pub visible: bool,
    pub text: String,
    pub position: Vec2,
    /// Seconds accumulated while hovering the current target.
    pub hover_timer: f32,
    pub hide_grace_timer: f32,
    pub pending_text: Option<String>,
    pub pending_position: Vec2,
}

/// Hover delay before showing tooltip (seconds).
pub const TOOLTIP_HOVER_DELAY_SECS: f32 = 0.45;

/// Brief grace after hover ends so the popup does not flicker off between frames.
pub const TOOLTIP_HIDE_GRACE_SECS: f32 = 0.12;

impl DevTooltipState {
    pub fn show(&mut self, text: String, position: Vec2) {
        self.visible = true;
        self.text = text;
        self.position = position;
        self.pending_text = None;
        self.hover_timer = 0.0;
        self.hide_grace_timer = 0.0;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.text.clear();
        self.pending_text = None;
        self.hover_timer = 0.0;
        self.hide_grace_timer = 0.0;
    }

    pub fn queue_hover(&mut self, text: String, position: Vec2, delta_secs: f32) {
        self.hide_grace_timer = 0.0;
        if self.visible && self.text == text {
            self.position = position;
            return;
        }
        if self.pending_text.as_deref() != Some(text.as_str()) {
            self.pending_text = Some(text);
            self.pending_position = position;
            self.hover_timer = 0.0;
            self.visible = false;
            return;
        }
        self.pending_position = position;
        self.hover_timer += delta_secs;
        if self.hover_timer >= TOOLTIP_HOVER_DELAY_SECS {
            self.show(text, position);
        }
    }

    pub fn tick_hide_grace(&mut self, delta_secs: f32) -> bool {
        self.hide_grace_timer += delta_secs;
        self.hide_grace_timer >= TOOLTIP_HIDE_GRACE_SECS
    }
}
