//! Floating gameplay window palette (Slice 4) — Tier 2 chrome.
//!
//! Derived from the bottom HUD bronze/charcoal family at lower contrast so
//! draggable panels stay lighter than the permanent HUD band.

use bevy::prelude::*;

use super::super::styles::{HUD_RECESSED_CORE, HUD_RECESSED_FACE};

/// Outer bronze hairline around gameplay floating windows.
pub const WINDOW_BORDER: Color = Color::srgba(0.48, 0.39, 0.30, 0.62);
/// Narrow dark groove between the outer frame and interior surface.
pub const WINDOW_INNER_GROOVE: Color = Color::srgba(0.008, 0.010, 0.014, 0.92);
/// Main panel interior.
pub const WINDOW_BG: Color = HUD_RECESSED_CORE;
/// Title rail background — slightly lifted from the body.
pub const WINDOW_TITLE_BG: Color = Color::srgba(0.024, 0.028, 0.034, 0.96);
/// Bronze separator under the title rail.
pub const WINDOW_TITLE_ACCENT: Color = Color::srgba(0.48, 0.39, 0.30, 0.48);
/// Recessed subsection wells (inventory grid, accepted items, etc.).
pub const WINDOW_SECTION_BG: Color = HUD_RECESSED_FACE;
/// Hairline between matrix rows / section groups.
pub const WINDOW_ROW_SEPARATOR: Color = Color::srgba(0.16, 0.14, 0.12, 0.42);

pub const WINDOW_BORDER_PX: f32 = 1.0;
pub const WINDOW_GROOVE_PX: f32 = 1.0;
pub const WINDOW_CORNER_RADIUS_PX: f32 = 4.0;
pub const WINDOW_BODY_PADDING_PX: f32 = 10.0;
pub const WINDOW_SECTION_GAP_PX: f32 = 8.0;
pub const WINDOW_SECTION_PADDING_PX: f32 = 8.0;
pub const WINDOW_CLOSE_BUTTON_MIN_PX: f32 = 24.0;
