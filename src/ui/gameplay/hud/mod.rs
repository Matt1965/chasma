//! Chasma bottom HUD chrome, shared geometry helpers, and stat bars.

mod assets;
mod bars;
mod depth;
mod frames;
mod geometry;
mod section;

pub use assets::{
    HUD_ENDCAP_LEFT_PATH, HUD_ENDCAP_RIGHT_PATH, HUD_ENDCAP_SPIRE_PATH, HUD_PLATE_BACKING_PATH,
    HUD_PLATE_FRAME_PATH, HudFrameBackground, HudUiAssets, spawn_hud_ornaments,
};
pub use bars::{
    HudStatBarFill, HudStatBarId, HudStatBarValueText, hp_bar_color, nutrition_bar_color,
    spawn_stat_bar_row, sync_stat_bar,
};
pub use depth::{
    HudButtonShellState, hud_button_depth_style, hud_button_shell_style, hud_inset_fill_style,
    hud_inset_rim_style, hud_quiet_raised_style, hud_raised_bevel_border, hud_raised_face_color,
    hud_recessed_bevel_border, hud_recessed_fill_style, hud_recessed_well_style,
    hud_roster_card_style, hud_roster_ghost_slot_style, hud_stat_track_style,
    spawn_hud_button_top_highlight,
};
pub use frames::{HudPlateFrame, HudPlateSection, spawn_hud_plate_frames, sync_hud_plate_frames};
pub use geometry::{
    HudEndcapLeft, HudEndcapRight, HudEndcapSpire, HudViewportGeometry,
    apply_hud_viewport_geometry, compute_hud_viewport_geometry, measure_hud_viewport_geometry,
    roster_content_width, visible_roster_card_count,
};
pub use section::{
    hud_flex_section_node, hud_plate_colors, hud_section_node,
    hud_section_node_with_horizontal_padding, spawn_hud_utility_group_divider,
};

#[cfg(test)]
mod tests;
