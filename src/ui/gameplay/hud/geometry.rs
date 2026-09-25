//! Responsive bottom HUD geometry — single authority for section sizing.
//!
//! Slice 1 frame art and Slice 2 palette read these numbers; they do not
//! invent their own width/height rules.

use bevy::prelude::*;

use super::super::roster_scroll::ROSTER_NAV_BUTTON_WIDTH_PX;
use super::super::styles::{
    HUD_ENDCAP_LEFT_WIDTH_PX, HUD_ENDCAP_RIGHT_WIDTH_PX, HUD_FRAME_BOTTOM_PX, HUD_FRAME_TOP_PX,
    HUD_HEIGHT_PX, HUD_PORTRAIT_WIDTH_PX, HUD_ROSTER_CARD_GAP_PX, HUD_ROSTER_MIN_WIDTH_PX,
    HUD_ROSTER_SLOT_WIDTH_PX, HUD_SECTION_PADDING_PX, HUD_SOURCE_BAND_HEIGHT_PX,
};

/// Viewport width the mockup proportions were measured against.
pub const HUD_REFERENCE_VIEWPORT_WIDTH: f32 = 1920.0;
pub const HUD_REFERENCE_VIEWPORT_HEIGHT: f32 = 1080.0;

/// Visual share targets from the mockup (not hard mandates).
pub const HUD_SECTION_SELECTED_SHARE: f32 = 0.23;
pub const HUD_SECTION_ROSTER_SHARE: f32 = 0.39;
pub const HUD_SECTION_COMMANDS_SHARE: f32 = 0.22;
pub const HUD_SECTION_UTILITY_SHARE: f32 = 0.13;

pub const HUD_HEIGHT_MIN_PX: f32 = 168.0;
pub const HUD_HEIGHT_MAX_PX: f32 = HUD_HEIGHT_PX;

pub const HUD_SELECTED_MIN_PX: f32 = 280.0;
pub const HUD_SELECTED_MAX_PX: f32 = 400.0;
pub const HUD_COMMAND_MIN_PX: f32 = 300.0;
pub const HUD_COMMAND_MAX_PX: f32 = 420.0;
pub const HUD_UTILITY_MIN_PX: f32 = 132.0;
pub const HUD_UTILITY_MAX_PX: f32 = 176.0;

pub const HUD_ROSTER_CARD_MIN_PX: f32 = 72.0;
pub const HUD_ROSTER_CARD_MAX_PX: f32 = HUD_ROSTER_SLOT_WIDTH_PX;

pub const HUD_PORTRAIT_MIN_PX: f32 = 96.0;
pub const HUD_PORTRAIT_MAX_PX: f32 = HUD_PORTRAIT_WIDTH_PX;

pub const HUD_COMMAND_BUTTON_MAX_PX: f32 = 72.0;
pub const HUD_COMMAND_BUTTON_GAP_PX: f32 = 6.0;

const MOCKUP_BAND_TO_WIDTH: f32 = HUD_SOURCE_BAND_HEIGHT_PX / 1983.0;
const VIEWPORT_HEIGHT_CAP_SHARE: f32 = 0.17;

/// Resolved HUD layout for the current viewport.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct HudViewportGeometry {
    pub viewport: Vec2,
    pub hud_height: f32,
    pub content_height: f32,
    pub art_scale: f32,
    pub endcap_left_width: f32,
    pub endcap_right_width: f32,
    pub frame_top: f32,
    pub frame_bottom: f32,
    pub content_row_width: f32,
    pub selected_width: f32,
    pub roster_width: f32,
    pub command_width: f32,
    pub utility_width: f32,
    pub portrait_width: f32,
    pub card_width: f32,
    pub card_gap: f32,
    pub command_button_width: f32,
    pub roster_viewport_width: f32,
    pub visible_roster_slots: u32,
}

impl Default for HudViewportGeometry {
    fn default() -> Self {
        compute_hud_viewport_geometry(Vec2::new(
            HUD_REFERENCE_VIEWPORT_WIDTH,
            HUD_REFERENCE_VIEWPORT_HEIGHT,
        ))
    }
}

impl HudViewportGeometry {
    pub fn is_reference_viewport(&self) -> bool {
        (self.viewport.x - HUD_REFERENCE_VIEWPORT_WIDTH).abs() < 0.5
            && (self.viewport.y - HUD_REFERENCE_VIEWPORT_HEIGHT).abs() < 0.5
    }
}

/// Derive the HUD band height from viewport dimensions (clamped, not linear width scale).
pub fn compute_hud_height(viewport: Vec2) -> f32 {
    let target = (viewport.x * MOCKUP_BAND_TO_WIDTH).clamp(HUD_HEIGHT_MIN_PX, HUD_HEIGHT_MAX_PX);
    if viewport.y < 720.0 {
        target.min((viewport.y * VIEWPORT_HEIGHT_CAP_SHARE).max(HUD_HEIGHT_MIN_PX))
    } else {
        target
    }
}

pub fn hud_art_scale(hud_height: f32) -> f32 {
    hud_height / HUD_SOURCE_BAND_HEIGHT_PX
}

pub fn scaled_endcap_left_width(hud_height: f32) -> f32 {
    ENDCAP_LEFT_SOURCE_WIDTH * hud_art_scale(hud_height)
}

pub fn scaled_endcap_right_width(hud_height: f32) -> f32 {
    ENDCAP_RIGHT_SOURCE_WIDTH * hud_art_scale(hud_height)
}

pub fn hud_content_height(hud_height: f32) -> f32 {
    let scale = hud_art_scale(hud_height);
    hud_height - HUD_FRAME_TOP_PX * scale - HUD_FRAME_BOTTOM_PX * scale
}

const ENDCAP_LEFT_SOURCE_WIDTH: f32 =
    HUD_ENDCAP_LEFT_WIDTH_PX / (HUD_HEIGHT_PX / HUD_SOURCE_BAND_HEIGHT_PX);
const ENDCAP_RIGHT_SOURCE_WIDTH: f32 =
    HUD_ENDCAP_RIGHT_WIDTH_PX / (HUD_HEIGHT_PX / HUD_SOURCE_BAND_HEIGHT_PX);

#[derive(Debug, Clone, Copy, PartialEq)]
struct SectionWidths {
    selected: f32,
    roster: f32,
    command: f32,
    utility: f32,
}

/// Distribute the bottom-bar row between the four sections.
pub fn distribute_section_widths(content_row_width: f32) -> SectionWidths {
    let usable = content_row_width.max(0.0);
    if usable <= 0.0 {
        return SectionWidths {
            selected: 0.0,
            roster: 0.0,
            command: 0.0,
            utility: 0.0,
        };
    }

    let mut selected =
        (usable * HUD_SECTION_SELECTED_SHARE).clamp(HUD_SELECTED_MIN_PX, HUD_SELECTED_MAX_PX);
    let mut command =
        (usable * HUD_SECTION_COMMANDS_SHARE).clamp(HUD_COMMAND_MIN_PX, HUD_COMMAND_MAX_PX);
    let mut utility =
        (usable * HUD_SECTION_UTILITY_SHARE).clamp(HUD_UTILITY_MIN_PX, HUD_UTILITY_MAX_PX);
    let roster = usable - selected - command - utility;

    if roster >= HUD_ROSTER_MIN_WIDTH_PX {
        return SectionWidths {
            selected,
            roster,
            command,
            utility,
        };
    }

    let deficit = HUD_ROSTER_MIN_WIDTH_PX - roster;
    let sel_slack = (selected - HUD_SELECTED_MIN_PX).max(0.0);
    let cmd_slack = (command - HUD_COMMAND_MIN_PX).max(0.0);
    let util_slack = (utility - HUD_UTILITY_MIN_PX).max(0.0);
    let total_slack = sel_slack + cmd_slack + util_slack;

    if total_slack > 0.0 {
        selected -= deficit * (sel_slack / total_slack);
        command -= deficit * (cmd_slack / total_slack);
        utility -= deficit * (util_slack / total_slack);
    } else {
        let fixed_total = selected + command + utility;
        if fixed_total > 0.0 {
            let scale = (usable - HUD_ROSTER_MIN_WIDTH_PX).max(0.0) / fixed_total;
            selected *= scale;
            command *= scale;
            utility *= scale;
        }
    }

    SectionWidths {
        selected,
        roster: (usable - selected - command - utility).max(0.0),
        command,
        utility,
    }
}

pub fn roster_viewport_width(roster_section_width: f32) -> f32 {
    let inner = roster_section_width - 2.0 * HUD_SECTION_PADDING_PX;
    let nav_total = 2.0 * ROSTER_NAV_BUTTON_WIDTH_PX + 4.0;
    (inner - nav_total).max(0.0)
}

pub fn visible_roster_card_count(viewport_width: f32, card_width: f32, card_gap: f32) -> u32 {
    if viewport_width <= 0.0 || card_width <= 0.0 {
        return 0;
    }
    let stride = card_width + card_gap;
    ((viewport_width + card_gap) / stride).floor().max(0.0) as u32
}

pub fn roster_content_width(card_count: usize, card_width: f32, card_gap: f32) -> f32 {
    if card_count == 0 {
        return 0.0;
    }
    card_count as f32 * card_width + (card_count.saturating_sub(1) as f32) * card_gap
}

pub fn command_button_width(command_section_width: f32, button_count: usize) -> f32 {
    if button_count == 0 {
        return 0.0;
    }
    let inner = command_section_width - 2.0 * HUD_SECTION_PADDING_PX;
    let gap_total = HUD_COMMAND_BUTTON_GAP_PX * (button_count.saturating_sub(1) as f32);
    let raw = (inner - gap_total) / button_count as f32;
    raw.clamp(48.0, HUD_COMMAND_BUTTON_MAX_PX)
}

pub fn portrait_width_for_selected_section(selected_width: f32) -> f32 {
    (selected_width * 0.31).clamp(HUD_PORTRAIT_MIN_PX, HUD_PORTRAIT_MAX_PX)
}

pub fn compute_hud_viewport_geometry(viewport: Vec2) -> HudViewportGeometry {
    let hud_height = compute_hud_height(viewport);
    let art_scale = hud_art_scale(hud_height);
    let endcap_left_width = scaled_endcap_left_width(hud_height);
    let endcap_right_width = scaled_endcap_right_width(hud_height);
    let frame_top = HUD_FRAME_TOP_PX * art_scale;
    let frame_bottom = HUD_FRAME_BOTTOM_PX * art_scale;
    let content_row_width = (viewport.x - endcap_left_width - endcap_right_width).max(0.0);
    let sections = distribute_section_widths(content_row_width);
    let card_width = HUD_ROSTER_CARD_MAX_PX.clamp(HUD_ROSTER_CARD_MIN_PX, HUD_ROSTER_CARD_MAX_PX);
    let card_gap = HUD_ROSTER_CARD_GAP_PX;
    let roster_viewport_width = roster_viewport_width(sections.roster);
    let visible_roster_slots =
        visible_roster_card_count(roster_viewport_width, card_width, card_gap);
    let command_button_width = command_button_width(sections.command, 5);
    let portrait_width = portrait_width_for_selected_section(sections.selected);

    HudViewportGeometry {
        viewport,
        hud_height,
        content_height: hud_content_height(hud_height),
        art_scale,
        endcap_left_width,
        endcap_right_width,
        frame_top,
        frame_bottom,
        content_row_width,
        selected_width: sections.selected,
        roster_width: sections.roster,
        command_width: sections.command,
        utility_width: sections.utility,
        portrait_width,
        card_width,
        card_gap,
        command_button_width,
        roster_viewport_width,
        visible_roster_slots,
    }
}

/// Track the primary window and refresh [`HudViewportGeometry`] on resize.
pub fn measure_hud_viewport_geometry(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut geometry: ResMut<HudViewportGeometry>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let viewport = Vec2::new(window.width(), window.height());
    if geometry.viewport == viewport {
        return;
    }
    *geometry = compute_hud_viewport_geometry(viewport);
}

/// Apply resolved geometry to the spawned HUD tree before UI layout runs.
pub fn apply_hud_viewport_geometry(world: &mut World) {
    let Some(geom) = world.get_resource::<HudViewportGeometry>().cloned() else {
        return;
    };

    for mut node in world
        .query_filtered::<&mut Node, With<super::super::layout::GameplayHudRoot>>()
        .iter_mut(world)
    {
        node.height = Val::Px(geom.hud_height);
    }
    for mut node in world
        .query_filtered::<&mut Node, With<super::super::layout::BottomBar>>()
        .iter_mut(world)
    {
        node.left = Val::Px(geom.endcap_left_width);
        node.right = Val::Px(geom.endcap_right_width);
        node.top = Val::Px(geom.frame_top);
        node.bottom = Val::Px(geom.frame_bottom);
    }
    for mut node in world
        .query_filtered::<&mut Node, With<super::super::selected_unit_panel::SelectedUnitPanelRoot>>()
        .iter_mut(world)
    {
        node.width = Val::Px(geom.selected_width);
    }
    for mut node in world
        .query_filtered::<&mut Node, With<super::super::squad_panel::SquadPanelRoot>>()
        .iter_mut(world)
    {
        node.min_width = Val::Px(HUD_ROSTER_MIN_WIDTH_PX);
        node.flex_grow = 1.0;
        node.flex_basis = Val::Px(0.0);
    }
    for mut node in world
        .query_filtered::<&mut Node, With<super::super::command_panel::CommandPanelRoot>>()
        .iter_mut(world)
    {
        node.width = Val::Px(geom.command_width);
    }
    for mut node in world
        .query_filtered::<&mut Node, With<super::super::utility_panel::UtilityPanelRoot>>()
        .iter_mut(world)
    {
        node.width = Val::Px(geom.utility_width);
    }
    for mut node in world
        .query_filtered::<&mut Node, With<super::super::selected_unit_panel::SelectedUnitPortraitFrame>>()
        .iter_mut(world)
    {
        node.width = Val::Px(geom.portrait_width);
    }
    for mut node in world
        .query_filtered::<&mut Node, (
            With<super::super::command_panel::HudCommandButton>,
            Without<super::super::selected_unit_panel::SelectedUnitPortraitFrame>,
        )>()
        .iter_mut(world)
    {
        node.flex_grow = 0.0;
        node.flex_basis = Val::Px(geom.command_button_width);
        node.width = Val::Px(geom.command_button_width);
    }
    for mut node in world
        .query_filtered::<&mut Node, With<super::super::squad_panel::SquadEntryList>>()
        .iter_mut(world)
    {
        node.column_gap = Val::Px(geom.card_gap);
    }
    for mut node in world
        .query_filtered::<&mut Node, With<HudEndcapLeft>>()
        .iter_mut(world)
    {
        node.width = Val::Px(geom.endcap_left_width);
    }
    for mut node in world
        .query_filtered::<&mut Node, With<HudEndcapRight>>()
        .iter_mut(world)
    {
        node.width = Val::Px(geom.endcap_right_width);
    }
    for mut node in world
        .query_filtered::<&mut Node, With<HudEndcapSpire>>()
        .iter_mut(world)
    {
        node.right = Val::Px(geom.endcap_right_width);
        node.width = Val::Px(ENDCAP_SPIRE_SOURCE_WIDTH * geom.art_scale);
        node.height = Val::Px(ENDCAP_SPIRE_SOURCE_HEIGHT * geom.art_scale);
    }
    for (frame, mut node) in world
        .query::<(&super::frames::HudPlateFrame, &mut Node)>()
        .iter_mut(world)
    {
        let (top, bottom) = frame.section.insets_px();
        node.top = Val::Px(top);
        node.height = Val::Px((geom.hud_height - top - bottom).max(0.0));
    }
}

const ENDCAP_SPIRE_SOURCE_WIDTH: f32 = 21.0;
const ENDCAP_SPIRE_SOURCE_HEIGHT: f32 = 96.0;

#[derive(Component, Debug)]
pub struct HudEndcapLeft;

#[derive(Component, Debug)]
pub struct HudEndcapRight;

#[derive(Component, Debug)]
pub struct HudEndcapSpire;

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry_at(width: f32) -> HudViewportGeometry {
        compute_hud_viewport_geometry(Vec2::new(width, 1080.0))
    }

    #[test]
    fn section_widths_fit_content_row() {
        for width in [1366.0, 1920.0, 2560.0] {
            let geom = geometry_at(width);
            let total =
                geom.selected_width + geom.command_width + geom.utility_width + geom.roster_width;
            assert!(
                (total - geom.content_row_width).abs() < 0.5,
                "at {width}px sections should sum to content row"
            );
        }
    }

    #[test]
    fn narrow_viewport_respects_functional_minimums() {
        let geom = geometry_at(1366.0);
        assert!(geom.selected_width >= HUD_SELECTED_MIN_PX - 0.5);
        assert!(geom.command_width >= HUD_COMMAND_MIN_PX - 0.5);
        assert!(geom.utility_width >= HUD_UTILITY_MIN_PX - 0.5);
        assert!(geom.roster_width >= HUD_ROSTER_MIN_WIDTH_PX - 0.5);
    }

    #[test]
    fn reference_viewport_keeps_roster_largest() {
        let geom = geometry_at(HUD_REFERENCE_VIEWPORT_WIDTH);
        assert!(geom.roster_width > geom.selected_width);
        assert!(geom.roster_width > geom.command_width);
        assert!(geom.roster_width > geom.utility_width);
        assert!(geom.selected_width > geom.utility_width);
    }

    #[test]
    fn wide_viewport_roster_absorbs_extra_width() {
        let normal = geometry_at(1920.0);
        let wide = geometry_at(2560.0);
        assert!(wide.roster_width > normal.roster_width);
        assert!(wide.selected_width <= HUD_SELECTED_MAX_PX + 0.5);
        assert!(wide.command_width <= HUD_COMMAND_MAX_PX + 0.5);
        assert!(wide.utility_width <= HUD_UTILITY_MAX_PX + 0.5);
        assert!(normal.selected_width <= wide.selected_width);
        assert!(normal.command_width <= wide.command_width);
    }

    #[test]
    fn visible_roster_slots_grow_with_width() {
        let narrow = geometry_at(1366.0);
        let wide = geometry_at(2560.0);
        assert!(wide.visible_roster_slots > narrow.visible_roster_slots);
    }

    #[test]
    fn card_width_stays_clamped() {
        for width in [1366.0, 1920.0, 2560.0] {
            let geom = geometry_at(width);
            assert!(geom.card_width >= HUD_ROSTER_CARD_MIN_PX);
            assert!(geom.card_width <= HUD_ROSTER_CARD_MAX_PX);
        }
    }

    #[test]
    fn hud_height_is_clamped_not_linear_with_width() {
        let narrow = compute_hud_height(Vec2::new(1366.0, 768.0));
        let wide = compute_hud_height(Vec2::new(2560.0, 1440.0));
        let reference = compute_hud_height(Vec2::new(1920.0, 1080.0));
        assert!(narrow >= HUD_HEIGHT_MIN_PX);
        assert!(wide <= HUD_HEIGHT_MAX_PX);
        assert!(reference > narrow);
        assert!((wide - reference).abs() < 8.0);
    }

    #[test]
    fn command_buttons_do_not_balloon_on_wide_screens() {
        let geom = geometry_at(2560.0);
        assert!(geom.command_button_width <= HUD_COMMAND_BUTTON_MAX_PX + 0.5);
    }

    /// Horizontal layout passes must not drift accepted vertical HUD geometry.
    #[test]
    fn horizontal_cleanup_preserves_locked_vertical_geometry() {
        use super::super::super::squad_panel::{
            HUD_ROSTER_CARD_HP_HEIGHT_PX, HUD_ROSTER_CARD_LABEL_HEIGHT_PX,
            HUD_ROSTER_HEADER_HEIGHT_PX,
        };
        use super::super::super::styles::{
            BOTTOM_BAR_HEIGHT_PX, HUD_COMMAND_BUTTON_HEIGHT_PERCENT, HUD_CONTENT_HEIGHT_PX,
            HUD_FRAME_BOTTOM_PX, HUD_FRAME_TOP_PX, HUD_HEIGHT_PX, HUD_UTILITY_BUTTON_HEIGHT_PX,
        };
        use super::super::section::{hud_flex_section_node, hud_section_node};

        assert_eq!(HUD_HEIGHT_PX, 196.0);
        assert_eq!(BOTTOM_BAR_HEIGHT_PX, HUD_HEIGHT_PX);
        assert_eq!(
            HUD_CONTENT_HEIGHT_PX,
            HUD_HEIGHT_PX - HUD_FRAME_TOP_PX - HUD_FRAME_BOTTOM_PX
        );
        assert_eq!(HUD_COMMAND_BUTTON_HEIGHT_PERCENT, 74.0);
        assert_eq!(HUD_UTILITY_BUTTON_HEIGHT_PX, 36.0);
        assert_eq!(HUD_ROSTER_HEADER_HEIGHT_PX, 24.0);
        assert_eq!(HUD_ROSTER_CARD_HP_HEIGHT_PX, 8.0);
        assert_eq!(HUD_ROSTER_CARD_LABEL_HEIGHT_PX, 15.0);

        for node in [hud_section_node(320.0), hud_flex_section_node(180.0)] {
            assert_eq!(node.padding.top, Val::Px(0.0));
            assert_eq!(node.padding.bottom, Val::Px(0.0));
            assert_eq!(node.height, Val::Percent(100.0));
        }

        let reference = Vec2::new(HUD_REFERENCE_VIEWPORT_WIDTH, HUD_REFERENCE_VIEWPORT_HEIGHT);
        let geom = compute_hud_viewport_geometry(reference);
        assert_eq!(geom.hud_height, compute_hud_height(reference));
        assert_eq!(geom.content_height, hud_content_height(geom.hud_height));
    }
}
