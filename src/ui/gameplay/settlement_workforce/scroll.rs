//! Workforce worker-row scrolling — bounded viewport, wheel input, visible scrollbar.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition, ScrollPosition};
use bevy::window::PrimaryWindow;

use super::panel::{
    SettlementWorkforceMatrixContentHost, SettlementWorkforceMatrixRowsScroll,
    SettlementWorkforceVerticalScrollbar, SettlementWorkforceVerticalScrollbarThumb,
};
use super::state::SettlementWorkforcePanelState;

pub const WORKFORCE_SCROLL_WHEEL_LINE_PX: f32 = 24.0;
pub const WORKFORCE_SCROLLBAR_MIN_THUMB_PX: f32 = 24.0;
pub const WORKFORCE_SCROLLBAR_TRACK_WIDTH_PX: f32 = 10.0;

/// Client-local worker-row scroll metrics and offset (authoritative for tests).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct SettlementWorkforceScrollState {
    pub scroll_offset_y: f32,
    pub viewport_height: f32,
    pub content_height: f32,
}

impl Default for SettlementWorkforceScrollState {
    fn default() -> Self {
        Self {
            scroll_offset_y: 0.0,
            viewport_height: 0.0,
            content_height: 0.0,
        }
    }
}

impl SettlementWorkforceScrollState {
    pub fn max_scroll_y(&self) -> f32 {
        max_scroll_y(self.viewport_height, self.content_height)
    }

    pub fn has_overflow(&self) -> bool {
        self.max_scroll_y() > 0.0
    }

    pub fn clamp_offset(&mut self) {
        self.scroll_offset_y = clamp_scroll_offset_y(
            self.scroll_offset_y,
            self.viewport_height,
            self.content_height,
        );
    }

    pub fn reset(&mut self) {
        self.scroll_offset_y = 0.0;
    }

    pub fn apply_wheel_delta(&mut self, wheel_delta_y: f32) {
        if wheel_delta_y.abs() <= f32::EPSILON {
            return;
        }
        // Positive wheel delta scrolls up (decrease offset); negative scrolls down.
        self.scroll_offset_y -= wheel_delta_y * WORKFORCE_SCROLL_WHEEL_LINE_PX;
        self.clamp_offset();
    }

    pub fn thumb_metrics(&self) -> (f32, f32) {
        scrollbar_thumb_metrics(
            self.viewport_height,
            self.content_height,
            self.scroll_offset_y,
            WORKFORCE_SCROLLBAR_MIN_THUMB_PX,
        )
    }
}

pub fn max_scroll_y(viewport_height: f32, content_height: f32) -> f32 {
    (content_height - viewport_height).max(0.0)
}

pub fn clamp_scroll_offset_y(offset_y: f32, viewport_height: f32, content_height: f32) -> f32 {
    offset_y.clamp(0.0, max_scroll_y(viewport_height, content_height))
}

pub fn scrollbar_thumb_metrics(
    viewport_height: f32,
    content_height: f32,
    scroll_offset_y: f32,
    min_thumb_length: f32,
) -> (f32, f32) {
    if viewport_height <= 0.0 || content_height <= viewport_height {
        return (viewport_height.max(min_thumb_length), 0.0);
    }
    let track = viewport_height;
    let thumb_height = (track * (viewport_height / content_height)).max(min_thumb_length);
    let travel = (track - thumb_height).max(0.0);
    let thumb_top = if content_height <= viewport_height {
        0.0
    } else {
        scroll_offset_y / max_scroll_y(viewport_height, content_height) * travel
    };
    (thumb_height, thumb_top)
}

pub struct SettlementWorkforceScrollPlugin;

impl Plugin for SettlementWorkforceScrollPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SettlementWorkforceScrollState>()
            .add_systems(
                PostUpdate,
                (
                    measure_settlement_workforce_scroll_state,
                    apply_settlement_workforce_scroll_position,
                    sync_settlement_workforce_vertical_scrollbar_visibility,
                    sync_settlement_workforce_scrollbar_thumb_layout,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    handle_settlement_workforce_scroll_wheel,
                    handle_settlement_workforce_scrollbar_track_click,
                )
                    .after(crate::ui::gameplay::input_gate::update_player_hud_hover_state),
            );
    }
}

pub fn reset_settlement_workforce_scroll(scroll_state: &mut SettlementWorkforceScrollState) {
    scroll_state.reset();
}

pub fn measure_settlement_workforce_scroll_state(
    panel: Res<SettlementWorkforcePanelState>,
    mut scroll_state: ResMut<SettlementWorkforceScrollState>,
    viewport_nodes: Query<&ComputedNode, With<SettlementWorkforceMatrixRowsScroll>>,
    content_nodes: Query<&ComputedNode, With<SettlementWorkforceMatrixContentHost>>,
    scroll_positions: Query<&ScrollPosition, With<SettlementWorkforceMatrixRowsScroll>>,
) {
    if !panel.open {
        scroll_state.viewport_height = 0.0;
        scroll_state.content_height = 0.0;
        scroll_state.reset();
        return;
    }

    if let Ok(viewport) = viewport_nodes.single() {
        scroll_state.viewport_height = viewport.size().y;
    }
    if let Ok(content) = content_nodes.single() {
        scroll_state.content_height = content.size().y;
    }

    if let Ok(scroll_position) = scroll_positions.single() {
        scroll_state.scroll_offset_y = scroll_position.y;
    }
    scroll_state.clamp_offset();
}

pub fn apply_settlement_workforce_scroll_position(
    panel: Res<SettlementWorkforcePanelState>,
    scroll_state: Res<SettlementWorkforceScrollState>,
    mut scroll_positions: Query<&mut ScrollPosition, With<SettlementWorkforceMatrixRowsScroll>>,
) {
    if !panel.open {
        return;
    }
    for mut scroll_position in &mut scroll_positions {
        scroll_position.y = scroll_state.scroll_offset_y;
    }
}

pub fn sync_settlement_workforce_vertical_scrollbar_visibility(
    panel: Res<SettlementWorkforcePanelState>,
    scroll_state: Res<SettlementWorkforceScrollState>,
    mut scrollbars: Query<&mut Visibility, With<SettlementWorkforceVerticalScrollbar>>,
) {
    let visible = panel.open && scroll_state.has_overflow();
    for mut visibility in &mut scrollbars {
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub fn sync_settlement_workforce_scrollbar_thumb_layout(
    panel: Res<SettlementWorkforcePanelState>,
    scroll_state: Res<SettlementWorkforceScrollState>,
    track_nodes: Query<&ComputedNode, With<SettlementWorkforceVerticalScrollbar>>,
    mut thumb_nodes: Query<&mut Node, With<SettlementWorkforceVerticalScrollbarThumb>>,
) {
    if !panel.open || !scroll_state.has_overflow() {
        return;
    }
    let Ok(track) = track_nodes.single() else {
        return;
    };
    let track_height = track.size().y;
    if track_height <= 1.0 {
        return;
    }
    let (thumb_height, thumb_top) = scrollbar_thumb_metrics(
        track_height,
        scroll_state.content_height,
        scroll_state.scroll_offset_y,
        WORKFORCE_SCROLLBAR_MIN_THUMB_PX,
    );
    for mut thumb in &mut thumb_nodes {
        thumb.height = Val::Px(thumb_height);
        thumb.top = Val::Px(thumb_top);
        thumb.width = Val::Percent(100.0);
    }
}

pub fn handle_settlement_workforce_scrollbar_track_click(
    panel: Res<SettlementWorkforcePanelState>,
    mut scroll_state: ResMut<SettlementWorkforceScrollState>,
    mut scroll_positions: Query<&mut ScrollPosition, With<SettlementWorkforceMatrixRowsScroll>>,
    tracks: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode),
        (
            Changed<Interaction>,
            With<SettlementWorkforceVerticalScrollbar>,
        ),
    >,
) {
    if !panel.open {
        return;
    }
    for (interaction, relative, track) in &tracks {
        if *interaction != Interaction::Pressed || !scroll_state.has_overflow() {
            continue;
        }
        let track_height = track.size().y;
        if track_height <= 1.0 {
            continue;
        }
        let (thumb_height, _) = scrollbar_thumb_metrics(
            track_height,
            scroll_state.content_height,
            scroll_state.scroll_offset_y,
            WORKFORCE_SCROLLBAR_MIN_THUMB_PX,
        );
        let Some(normalized) = relative.normalized else {
            continue;
        };
        let travel = (track_height - thumb_height).max(0.0);
        let click_y = normalized.y.clamp(0.0, 1.0) * track_height;
        let target_top = (click_y - thumb_height * 0.5).clamp(0.0, travel);
        let ratio = if travel <= 0.0 {
            0.0
        } else {
            target_top / travel
        };
        scroll_state.scroll_offset_y = ratio * scroll_state.max_scroll_y();
        scroll_state.clamp_offset();
        for mut scroll_position in &mut scroll_positions {
            scroll_position.y = scroll_state.scroll_offset_y;
        }
    }
}

pub fn handle_settlement_workforce_scroll_wheel(
    panel: Res<SettlementWorkforcePanelState>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window, With<PrimaryWindow>>,
    registry: Res<crate::ui::gameplay::floating_window::FloatingGameplayWindowRegistry>,
    mut scroll_state: ResMut<SettlementWorkforceScrollState>,
    mut scroll_positions: Query<&mut ScrollPosition, With<SettlementWorkforceMatrixRowsScroll>>,
    mut hover: ResMut<crate::ui::gameplay::input_gate::PlayerHudHoverState>,
) {
    if !panel.open {
        return;
    }
    let wheel_delta = mouse_scroll.delta.y;
    let cursor = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position());
    let hovered = cursor.is_some_and(|point| {
        registry.pointer_inside_open_window(
            crate::ui::gameplay::floating_window::FloatingGameplayWindowId::SettlementWorkforce,
            point,
        )
    });
    if hovered {
        hover.blocks_camera_scroll = true;
    }
    if wheel_delta.abs() <= f32::EPSILON || !hovered {
        return;
    }

    scroll_state.apply_wheel_delta(wheel_delta);
    for mut scroll_position in &mut scroll_positions {
        scroll_position.y = scroll_state.scroll_offset_y;
    }
}
