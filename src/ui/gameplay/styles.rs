//! Shared Bevy UI styling for the player HUD (P-UI1).

use bevy::prelude::*;

pub use super::typography::{
    hud_body_font, hud_caption_font, hud_heading_font, hud_title_font, panel_body_font,
    panel_title_font,
};

// --- Shared bottom HUD vertical geometry ---------------------------------
//
// Every section derives its vertical size from these constants so panel joins
// cannot drift apart. Values come from the `chasma_ui.png` mockup, whose HUD
// band measures y 567..766 (199px) across a 1983px-wide composite -- almost
// exactly 10% of width, i.e. ~196px at a 1920px reference viewport.
// See `scripts/crop_chasma_hud_assets.py` for the measured landmarks.

/// Source mockup band height, in mockup pixels. Endcap aspect ratios key off it.
pub const HUD_SOURCE_BAND_HEIGHT_PX: f32 = 199.0;

/// Bottom HUD band height.
pub const HUD_HEIGHT_PX: f32 = 196.0;

/// Frame bevel thickness reserved at the top and bottom of the band. Section
/// content starts inside these so the continuous frame art is never covered.
pub const HUD_FRAME_TOP_PX: f32 = 12.0;
pub const HUD_FRAME_BOTTOM_PX: f32 = 14.0;

/// Vertical space available to section content between the frame bevels.
pub const HUD_CONTENT_HEIGHT_PX: f32 = HUD_HEIGHT_PX - HUD_FRAME_TOP_PX - HUD_FRAME_BOTTOM_PX;

/// Ornamental endcap source sizes (px in `chasma_ui.png`), scaled to band height.
const ENDCAP_LEFT_SOURCE: Vec2 = Vec2::new(54.0, 199.0);
const ENDCAP_RIGHT_SOURCE: Vec2 = Vec2::new(24.0, 199.0);
const ENDCAP_SPIRE_SOURCE: Vec2 = Vec2::new(21.0, 96.0);

/// Scale from mockup pixels to runtime pixels at [`HUD_HEIGHT_PX`].
pub const HUD_ART_SCALE: f32 = HUD_HEIGHT_PX / HUD_SOURCE_BAND_HEIGHT_PX;

pub const HUD_ENDCAP_LEFT_WIDTH_PX: f32 = ENDCAP_LEFT_SOURCE.x * HUD_ART_SCALE;
pub const HUD_ENDCAP_RIGHT_WIDTH_PX: f32 = ENDCAP_RIGHT_SOURCE.x * HUD_ART_SCALE;
pub const HUD_SPIRE_WIDTH_PX: f32 = ENDCAP_SPIRE_SOURCE.x * HUD_ART_SCALE;
pub const HUD_SPIRE_HEIGHT_PX: f32 = ENDCAP_SPIRE_SOURCE.y * HUD_ART_SCALE;

/// Nine-slice corner block of `plate_frame.png`, holding the chamfer and trim.
/// Corners are not scaled, so this is both the source and runtime corner size.
pub const HUD_PLATE_CORNER_PX: f32 = 32.0;

/// Length of the 45-degree cut across each plate corner, inside the corner
/// block. A point is outside the plate when `x + y < HUD_PLATE_CHAMFER_PX`.
/// Sized to match the mockup's left-nose taper (~31px) so plates do not read
/// as rectangles with tiny clipped corners.
pub const HUD_PLATE_CHAMFER_PX: f32 = 26.0;

/// Visible chamfer where two plates meet. Shallower than the outer cut so the
/// join stays closed, but large enough to read as a step at game distance.
pub const HUD_PLATE_JOIN_CHAMFER_PX: f32 = 8.0;

/// How far each plate reaches past a shared boundary, which is also how much of
/// its corner is clipped away to leave [`HUD_PLATE_JOIN_CHAMFER_PX`]. Both
/// plates extend, so a join is always double-backed.
pub const HUD_PLATE_JOIN_OVERLAP_PX: f32 = HUD_PLATE_CHAMFER_PX - HUD_PLATE_JOIN_CHAMFER_PX;

/// How far an outer plate tucks under its endcap so the top/bottom rails
/// continue into the nose instead of leaving a corner gap.
pub const HUD_ENDCAP_SEAT_PX: f32 = 10.0;

/// Continuous backing inset. Left must clear the tapered nose (opaque full
/// height only after ~x=32). Vertical inset must exceed the deepest plate
/// inset so the rectangle cannot poke outside a shorter plate.
pub const HUD_BACKING_INSET_X_PX: f32 = 32.0;
pub const HUD_BACKING_INSET_Y_PX: f32 = 14.0;

/// Sections abut directly; plate frames are drawn as root overlays.
pub const HUD_SECTION_GAP_PX: f32 = 0.0;
/// Layout tolerance for section abutment checks.
pub const HUD_DIVIDER_WIDTH_PX: f32 = 2.0;

/// Default horizontal padding inside HUD sections (vertical padding stays 0).
pub const HUD_SECTION_PADDING_X_PX: f32 = 8.0;
/// Outer-edge sections tuck content slightly closer to the frame endcaps.
pub const HUD_SECTION_PADDING_X_SELECTED_LEFT_PX: f32 = 6.0;
pub const HUD_SECTION_PADDING_X_UTILITY_RIGHT_PX: f32 = 6.0;
/// Back-compat alias for geometry helpers that mean horizontal inset.
pub const HUD_SECTION_PADDING_PX: f32 = HUD_SECTION_PADDING_X_PX;

// --- Section widths (reference at 1920px; runtime via HudViewportGeometry) -
//
// Selected, command, and utility sections are clamped to content needs; the
// roster absorbs remaining width so a wider viewport shows more cards.

/// Reference selected width at [`HUD_REFERENCE_VIEWPORT_WIDTH`].
pub const HUD_SELECTED_WIDTH_PX: f32 = 372.0;
/// Reference command width at [`HUD_REFERENCE_VIEWPORT_WIDTH`].
pub const HUD_COMMAND_WIDTH_PX: f32 = 400.0;
/// Reference utility width at [`HUD_REFERENCE_VIEWPORT_WIDTH`].
pub const HUD_UTILITY_WIDTH_PX: f32 = 168.0;
pub const HUD_ROSTER_MIN_WIDTH_PX: f32 = 180.0;

/// Portrait is a tall rounded plate filling most of the section content height.
pub const HUD_PORTRAIT_WIDTH_PX: f32 = 116.0;

/// Roster card footprint. Cards are taller than wide, as in the mockup.
pub const HUD_ROSTER_SLOT_WIDTH_PX: f32 = 84.0;
pub const HUD_ROSTER_CARD_GAP_PX: f32 = 6.0;

/// Command buttons take the same share of the content row as in the mockup
/// (118px buttons inside a 160px panel), so they scale with [`HUD_HEIGHT_PX`].
pub const HUD_COMMAND_BUTTON_HEIGHT_PERCENT: f32 = 74.0;

/// Utility rows are compact and fixed; four of them centre in the content row.
pub const HUD_UTILITY_BUTTON_HEIGHT_PX: f32 = 36.0;
pub const HUD_UTILITY_ROW_GAP_PX: f32 = 4.0;
pub const HUD_UTILITY_GROUP_DIVIDER_PX: f32 = 6.0;
pub const HUD_FIELDS_MENU_WIDTH_PX: f32 = HUD_UTILITY_WIDTH_PX - 8.0;

/// Legacy alias retained for the build-catalog anchor and pointer-capture rect.
pub const BOTTOM_BAR_HEIGHT_PX: f32 = HUD_HEIGHT_PX;

/// Horizontal padding inside floating panels (not the bottom HUD).
pub const PANEL_PADDING_PX: f32 = 10.0;

pub const BAR_BG: Color = Color::srgba(0.04, 0.06, 0.08, 0.82);
pub const PANEL_BG: Color = Color::srgba(0.08, 0.10, 0.12, 0.92);
pub const HUD_BAR_BG: Color = Color::srgba(0.05, 0.06, 0.08, 0.95);
pub const TEXT_PRIMARY: Color = Color::srgba(0.92, 0.95, 0.98, 1.0);
pub const TEXT_MUTED: Color = Color::srgba(0.65, 0.72, 0.78, 1.0);
pub const ACCENT_GREEN: Color = Color::srgba(0.35, 0.92, 0.42, 1.0);
pub const HUD_ACCENT_GOLD: Color = Color::srgba(0.82, 0.68, 0.28, 1.0);
pub const HUD_HP_FILL: Color = Color::srgba(0.78, 0.22, 0.18, 1.0);
pub const HUD_NUTRITION_FILL: Color = Color::srgba(0.82, 0.72, 0.18, 1.0);

// Legacy floating-panel button palette (not used by the bottom HUD).
pub const CMD_BTN_ENABLED_BG: Color = Color::srgba(0.12, 0.18, 0.24, 0.95);
pub const CMD_BTN_ENABLED_HOVER: Color = Color::srgba(0.18, 0.28, 0.36, 0.98);
pub const CMD_BTN_ENABLED_PRESSED: Color = Color::srgba(0.22, 0.38, 0.48, 1.0);
pub const CMD_BTN_DISABLED_BG: Color = Color::srgba(0.08, 0.1, 0.12, 0.55);
pub const CMD_BTN_BORDER: Color = Color::srgba(0.35, 0.55, 0.65, 0.75);
pub const CMD_BTN_ARMED_BG: Color = Color::srgba(0.15, 0.42, 0.22, 0.98);

// --- Bottom HUD interior palette (Slice 2) --------------------------------

/// Warm bronze/gold accent for armed commands and primary roster emphasis.
pub const HUD_ACTIVE_ACCENT: Color = Color::srgba(0.80, 0.64, 0.30, 1.0);

/// Raised control face at rest.
pub const HUD_RAISED_FACE: Color = Color::srgba(0.065, 0.058, 0.050, 0.96);
pub const HUD_RAISED_FACE_HOVER: Color = Color::srgba(0.085, 0.075, 0.062, 0.98);
pub const HUD_RAISED_FACE_PRESSED: Color = Color::srgba(0.038, 0.034, 0.030, 0.98);
pub const HUD_RAISED_FACE_ARMED: Color = Color::srgba(0.095, 0.078, 0.048, 0.98);
pub const HUD_RAISED_HIGHLIGHT: Color = Color::srgba(0.52, 0.43, 0.33, 0.82);
pub const HUD_RAISED_HIGHLIGHT_HOVER: Color = Color::srgba(0.62, 0.52, 0.40, 0.88);
pub const HUD_RAISED_SHADOW: Color = Color::srgba(0.12, 0.10, 0.085, 0.92);
pub const HUD_DISABLED_FACE: Color = Color::srgba(0.045, 0.044, 0.046, 0.50);
pub const HUD_DISABLED_BORDER: Color = Color::srgba(0.22, 0.20, 0.18, 0.45);

/// Recessed wells cut into the plate interior.
pub const HUD_RECESSED_FACE: Color = Color::srgba(0.024, 0.028, 0.034, 0.98);
pub const HUD_RECESSED_CORE: Color = Color::srgba(0.016, 0.019, 0.024, 0.98);
pub const HUD_RECESSED_SHADOW: Color = Color::srgba(0.008, 0.010, 0.014, 0.95);
pub const HUD_RECESSED_HIGHLIGHT: Color = Color::srgba(0.16, 0.14, 0.12, 0.55);

/// Roster card slot surfaces.
pub const HUD_ROSTER_SLOT_FACE: Color = HUD_RECESSED_FACE;
pub const HUD_ROSTER_SLOT_HOVER_FACE: Color = Color::srgba(0.040, 0.038, 0.034, 0.98);
pub const HUD_ROSTER_SLOT_SELECTED_FACE: Color = Color::srgba(0.052, 0.045, 0.034, 0.98);
pub const HUD_ROSTER_SLOT_BORDER: Color = Color::srgba(0.34, 0.28, 0.22, 0.55);
pub const HUD_ROSTER_SLOT_SELECTED_BORDER: Color = Color::srgba(0.72, 0.58, 0.28, 0.82);
pub const HUD_ROSTER_SLOT_PRIMARY_BORDER: Color = HUD_ACCENT_GOLD;

/// Stat bar tracks.
pub const HUD_TRACK_FACE: Color = Color::srgba(0.018, 0.022, 0.028, 0.98);
pub const HUD_TRACK_SHADOW: Color = Color::srgba(0.006, 0.008, 0.012, 0.95);
pub const HUD_TRACK_HIGHLIGHT: Color = Color::srgba(0.20, 0.18, 0.15, 0.42);

/// Inner content plate behind portraits, roster cards, and buttons.
pub const HUD_PLATE_BG: Color = HUD_RECESSED_CORE;
/// Outer rim on inset plates — slightly lighter bronze for bevel read.
pub const HUD_PLATE_RIM: Color = HUD_RECESSED_FACE;
/// Thin bronze trim around inner plates, matching the frame bevel.
pub const HUD_PLATE_TRIM: Color = Color::srgba(0.48, 0.39, 0.30, 0.90);
/// Inset shadow on recessed plate interiors.
pub const HUD_PLATE_SHADOW: Color = HUD_RECESSED_SHADOW;
/// Top-edge highlight on raised HUD buttons.
pub const HUD_BUTTON_TOP_HIGHLIGHT: Color = Color::srgba(0.68, 0.56, 0.40, 0.55);
/// Compact popup background behind the Fields menu.
pub const HUD_MENU_BG: Color = Color::srgba(0.05, 0.06, 0.075, 0.98);
/// Hairline between the utility panel's two button groups.
pub const HUD_DIVIDER_COLOR: Color = Color::srgba(0.40, 0.33, 0.25, 0.55);

pub fn command_button_bg(interaction: &Interaction, enabled: bool, armed: bool) -> BackgroundColor {
    if !enabled {
        return BackgroundColor(CMD_BTN_DISABLED_BG);
    }
    if armed {
        return BackgroundColor(CMD_BTN_ARMED_BG);
    }
    BackgroundColor(match *interaction {
        Interaction::Pressed => CMD_BTN_ENABLED_PRESSED,
        Interaction::Hovered => CMD_BTN_ENABLED_HOVER,
        Interaction::None => CMD_BTN_ENABLED_BG,
    })
}
