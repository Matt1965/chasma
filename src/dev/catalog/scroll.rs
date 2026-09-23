//! Catalog list scrolling — viewport measurement, wheel, scrollbar (virtual row window).

use bevy::ecs::system::SystemParam;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use super::super::dev_mode::DevModeState;
use super::super::input::DevPanelUi;
use super::super::panel::{
    DevArchetypeListViewport, DevArchetypeRow, DevCatalogListViewport, DevListRow,
};

pub const ROW_HEIGHT_PX: f32 = 20.0;
use super::super::window::{DevWindowId, DevWindowRegistry};

pub const CATALOG_ROW_GAP_PX: f32 = 2.0;
pub const CATALOG_SCROLLBAR_WIDTH_PX: f32 = 10.0;
pub const CATALOG_SCROLLBAR_MIN_THUMB_PX: f32 = 24.0;
pub const CATALOG_SCROLL_WHEEL_ROWS: usize = 3;

#[derive(Component, Debug)]
pub(crate) struct DevCatalogListScrollbar;

#[derive(Component, Debug)]
pub(crate) struct DevCatalogListScrollbarThumb;

#[derive(Component, Debug)]
pub(crate) struct DevArchetypeListScrollbar;

#[derive(Component, Debug)]
pub(crate) struct DevArchetypeListScrollbarThumb;

/// Cached counts and viewport sizes for scroll clamping (updated during list sync).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct CatalogScrollMetrics {
    pub definition_entry_count: usize,
    pub archetype_entry_count: usize,
    pub definition_viewport_height: f32,
    pub archetype_viewport_height: f32,
}

pub fn catalog_row_stride_px() -> f32 {
    ROW_HEIGHT_PX + CATALOG_ROW_GAP_PX
}

pub fn catalog_row_pool_capacity(max_list_height_px: f32) -> usize {
    let stride = catalog_row_stride_px();
    if stride <= 0.0 {
        return 1;
    }
    ((max_list_height_px / stride).ceil() as usize).max(1)
}

pub fn visible_row_count(viewport_height: f32) -> usize {
    let stride = catalog_row_stride_px();
    if viewport_height <= 0.0 || stride <= 0.0 {
        return 1;
    }
    (viewport_height / stride).floor().max(1.0) as usize
}

pub(crate) fn max_scroll_offset(entry_count: usize, visible_rows: usize) -> usize {
    entry_count.saturating_sub(visible_rows)
}

pub fn clamp_scroll_offset(offset: usize, entry_count: usize, visible_rows: usize) -> usize {
    offset.min(max_scroll_offset(entry_count, visible_rows))
}

pub fn scrollbar_thumb_metrics(
    track_height: f32,
    entry_count: usize,
    visible_rows: usize,
    scroll_offset: usize,
) -> (f32, f32) {
    if track_height <= 0.0 || entry_count <= visible_rows {
        return (track_height.max(CATALOG_SCROLLBAR_MIN_THUMB_PX), 0.0);
    }
    let thumb_height = (track_height * (visible_rows as f32 / entry_count as f32))
        .max(CATALOG_SCROLLBAR_MIN_THUMB_PX);
    let travel = (track_height - thumb_height).max(0.0);
    let max_offset = max_scroll_offset(entry_count, visible_rows);
    let thumb_top = if max_offset == 0 {
        0.0
    } else {
        scroll_offset as f32 / max_offset as f32 * travel
    };
    (thumb_height, thumb_top)
}

pub fn scroll_offset_from_thumb_top(
    thumb_top: f32,
    track_height: f32,
    entry_count: usize,
    visible_rows: usize,
) -> usize {
    let max_offset = max_scroll_offset(entry_count, visible_rows);
    if max_offset == 0 {
        return 0;
    }
    let (thumb_height, _) = scrollbar_thumb_metrics(track_height, entry_count, visible_rows, 0);
    let travel = (track_height - thumb_height).max(0.0);
    if travel <= 0.0 {
        return 0;
    }
    let ratio = (thumb_top / travel).clamp(0.0, 1.0);
    (ratio * max_offset as f32).round() as usize
}

#[derive(SystemParam)]
pub(crate) struct CatalogScrollWheelParams<'w> {
    pub registry: Res<'w, DevWindowRegistry>,
    pub dev_state: ResMut<'w, DevModeState>,
    pub metrics: Res<'w, CatalogScrollMetrics>,
    pub mouse_scroll: Res<'w, AccumulatedMouseScroll>,
    pub gate: ResMut<'w, super::super::DevModeInputGate>,
}

#[derive(SystemParam)]
pub(crate) struct CatalogScrollWheelQuery<'w, 's> {
    pub definition_viewports: Query<
        'w,
        's,
        &'static RelativeCursorPosition,
        (With<DevCatalogListViewport>, With<DevPanelUi>),
    >,
    pub archetype_viewports: Query<
        'w,
        's,
        &'static RelativeCursorPosition,
        (With<DevArchetypeListViewport>, With<DevPanelUi>),
    >,
}

pub fn measure_catalog_list_viewports(
    registry: Res<DevWindowRegistry>,
    mut metrics: ResMut<CatalogScrollMetrics>,
    definition_viewports: Query<&ComputedNode, With<DevCatalogListViewport>>,
    archetype_viewports: Query<&ComputedNode, With<DevArchetypeListViewport>>,
) {
    if !registry.is_visible(DevWindowId::Catalog) {
        metrics.definition_viewport_height = 0.0;
        metrics.archetype_viewport_height = 0.0;
        return;
    }
    if let Ok(node) = definition_viewports.single() {
        metrics.definition_viewport_height = node.size().y;
    }
    if let Ok(node) = archetype_viewports.single() {
        metrics.archetype_viewport_height = node.size().y;
    }
}

pub fn handle_catalog_list_scroll_wheel(
    mut params: CatalogScrollWheelParams,
    views: CatalogScrollWheelQuery,
) {
    if !params.registry.is_visible(DevWindowId::Catalog) || !params.dev_state.enabled {
        return;
    }
    let wheel_rows: i32 = if params.mouse_scroll.delta.y > 0.0 {
        CATALOG_SCROLL_WHEEL_ROWS as i32
    } else if params.mouse_scroll.delta.y < 0.0 {
        -(CATALOG_SCROLL_WHEEL_ROWS as i32)
    } else {
        0
    };
    if wheel_rows == 0 {
        return;
    }

    let metrics = *params.metrics;
    let definition_visible =
        visible_row_count(metrics.definition_viewport_height.max(catalog_row_stride_px()));
    let archetype_visible =
        visible_row_count(metrics.archetype_viewport_height.max(catalog_row_stride_px()));

    for relative in &views.definition_viewports {
        if relative.normalized.is_none() {
            continue;
        }
        params.gate.block_camera_scroll = true;
        let next = params.dev_state.list_scroll as i32 - wheel_rows;
        params.dev_state.list_scroll = clamp_scroll_offset(
            next.max(0) as usize,
            metrics.definition_entry_count,
            definition_visible,
        );
        return;
    }

    for relative in &views.archetype_viewports {
        if relative.normalized.is_none() {
            continue;
        }
        params.gate.block_camera_scroll = true;
        let next = params.dev_state.archetype_list_scroll as i32 - wheel_rows;
        params.dev_state.archetype_list_scroll = clamp_scroll_offset(
            next.max(0) as usize,
            metrics.archetype_entry_count,
            archetype_visible,
        );
        return;
    }
}

pub fn sync_catalog_list_scrollbars(
    registry: Res<DevWindowRegistry>,
    dev_state: Res<DevModeState>,
    metrics: Res<CatalogScrollMetrics>,
    mut definition_tracks: Query<
        (&mut Visibility, &ComputedNode),
        (With<DevCatalogListScrollbar>, Without<DevArchetypeListScrollbar>),
    >,
    mut archetype_tracks: Query<
        (&mut Visibility, &ComputedNode),
        (With<DevArchetypeListScrollbar>, Without<DevCatalogListScrollbar>),
    >,
    mut definition_thumbs: Query<
        &mut Node,
        (
            With<DevCatalogListScrollbarThumb>,
            Without<DevArchetypeListScrollbarThumb>,
        ),
    >,
    mut archetype_thumbs: Query<
        &mut Node,
        (
            With<DevArchetypeListScrollbarThumb>,
            Without<DevCatalogListScrollbarThumb>,
        ),
    >,
) {
    if !registry.is_visible(DevWindowId::Catalog) || !dev_state.enabled {
        return;
    }

    let definition_visible =
        visible_row_count(metrics.definition_viewport_height.max(catalog_row_stride_px()));
    let archetype_visible =
        visible_row_count(metrics.archetype_viewport_height.max(catalog_row_stride_px()));

    if let Ok((mut visibility, track)) = definition_tracks.single_mut() {
        let overflow = metrics.definition_entry_count > definition_visible;
        *visibility = if overflow {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let track_height = track.size().y;
        let (thumb_height, thumb_top) = scrollbar_thumb_metrics(
            track_height,
            metrics.definition_entry_count,
            definition_visible,
            dev_state.list_scroll,
        );
        for mut thumb in &mut definition_thumbs {
            thumb.height = Val::Px(thumb_height);
            thumb.top = Val::Px(thumb_top);
            thumb.width = Val::Percent(100.0);
        }
    }

    if let Ok((mut visibility, track)) = archetype_tracks.single_mut() {
        let overflow = metrics.archetype_entry_count > archetype_visible;
        *visibility = if overflow {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let track_height = track.size().y;
        let (thumb_height, thumb_top) = scrollbar_thumb_metrics(
            track_height,
            metrics.archetype_entry_count,
            archetype_visible,
            dev_state.archetype_list_scroll,
        );
        for mut thumb in &mut archetype_thumbs {
            thumb.height = Val::Px(thumb_height);
            thumb.top = Val::Px(thumb_top);
            thumb.width = Val::Percent(100.0);
        }
    }
}

pub fn handle_catalog_list_scrollbar_track_click(
    registry: Res<DevWindowRegistry>,
    metrics: Res<CatalogScrollMetrics>,
    mut dev_state: ResMut<DevModeState>,
    mut gate: ResMut<super::super::DevModeInputGate>,
    definition_tracks: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode),
        (
            Changed<Interaction>,
            With<DevCatalogListScrollbar>,
            Without<DevArchetypeListScrollbar>,
        ),
    >,
    archetype_tracks: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode),
        (
            Changed<Interaction>,
            With<DevArchetypeListScrollbar>,
            Without<DevCatalogListScrollbar>,
        ),
    >,
) {
    if !registry.is_visible(DevWindowId::Catalog) || !dev_state.enabled {
        return;
    }

    let definition_visible =
        visible_row_count(metrics.definition_viewport_height.max(catalog_row_stride_px()));
    let archetype_visible =
        visible_row_count(metrics.archetype_viewport_height.max(catalog_row_stride_px()));

    for (interaction, relative, track) in &definition_tracks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if metrics.definition_entry_count <= definition_visible {
            continue;
        }
        gate.block_gameplay_mouse = true;
        let Some(normalized) = relative.normalized else {
            continue;
        };
        let track_height = track.size().y;
        let (thumb_height, _) = scrollbar_thumb_metrics(
            track_height,
            metrics.definition_entry_count,
            definition_visible,
            dev_state.list_scroll,
        );
        let travel = (track_height - thumb_height).max(0.0);
        let click_y = normalized.y.clamp(0.0, 1.0) * track_height;
        let target_top = (click_y - thumb_height * 0.5).clamp(0.0, travel);
        dev_state.list_scroll = scroll_offset_from_thumb_top(
            target_top,
            track_height,
            metrics.definition_entry_count,
            definition_visible,
        );
        return;
    }

    for (interaction, relative, track) in &archetype_tracks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if metrics.archetype_entry_count <= archetype_visible {
            continue;
        }
        gate.block_gameplay_mouse = true;
        let Some(normalized) = relative.normalized else {
            continue;
        };
        let track_height = track.size().y;
        let (thumb_height, _) = scrollbar_thumb_metrics(
            track_height,
            metrics.archetype_entry_count,
            archetype_visible,
            dev_state.archetype_list_scroll,
        );
        let travel = (track_height - thumb_height).max(0.0);
        let click_y = normalized.y.clamp(0.0, 1.0) * track_height;
        let target_top = (click_y - thumb_height * 0.5).clamp(0.0, travel);
        dev_state.archetype_list_scroll = scroll_offset_from_thumb_top(
            target_top,
            track_height,
            metrics.archetype_entry_count,
            archetype_visible,
        );
    }
}

pub fn sync_catalog_list_row_visibility(
    registry: Res<DevWindowRegistry>,
    dev_state: Res<DevModeState>,
    metrics: Res<CatalogScrollMetrics>,
    mut definition_rows: Query<(&DevListRow, &mut Visibility)>,
    mut archetype_rows: Query<(&DevArchetypeRow, &mut Visibility), Without<DevListRow>>,
) {
    if !registry.is_visible(DevWindowId::Catalog) || !dev_state.enabled {
        return;
    }
    let definition_visible =
        visible_row_count(metrics.definition_viewport_height.max(catalog_row_stride_px()));
    let archetype_visible =
        visible_row_count(metrics.archetype_viewport_height.max(catalog_row_stride_px()));
    for (row, mut visibility) in &mut definition_rows {
        *visibility = if row.index < definition_visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (row, mut visibility) in &mut archetype_rows {
        *visibility = if row.index < archetype_visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_row_count_grows_with_viewport() {
        assert_eq!(visible_row_count(0.0), 1);
        assert_eq!(visible_row_count(catalog_row_stride_px() * 5.5), 5);
    }

    #[test]
    fn scroll_clamps_to_entry_count_minus_visible() {
        assert_eq!(max_scroll_offset(25, 10), 15);
        assert_eq!(clamp_scroll_offset(99, 25, 10), 15);
        assert_eq!(clamp_scroll_offset(3, 25, 10), 3);
    }

    #[test]
    fn clearing_search_restores_full_scroll_range() {
        let entry_count = 40;
        let visible = 12;
        assert_eq!(max_scroll_offset(entry_count, visible), 28);
        assert_eq!(clamp_scroll_offset(0, entry_count, visible), 0);
    }

    #[test]
    fn archetype_and_definition_scroll_offsets_are_independent() {
        let def_max = max_scroll_offset(30, 10);
        let arch_max = max_scroll_offset(8, 10);
        assert_eq!(def_max, 20);
        assert_eq!(arch_max, 0);
    }
}
