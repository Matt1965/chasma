//! Client-local unit selection and command routing (ADR-034 U9, ADR-038 U-UI2).
//!
//! Runtime-only interaction state: marquee select, shift modifiers, and multi-unit
//! move dispatch. Input collection and dispatch live in [`crate::client`]; this
//! module owns selection helpers, picking, and command issuance.

mod box_select;
mod commands;
mod controllability;
mod picking;
mod selection;
mod settings;
mod terrain_click;

pub use settings::PlayerInteractionSettings;
pub use terrain_click::{
    authoritative_position_at_global_xz, terrain_click_to_world_position, TerrainClickResult,
};

pub use box_select::{collect_units_in_screen_rect, normalized_screen_rect, BoxSelectDrag};
pub use controllability::{
    apply_selectable_filter, prune_non_commandable_from_selection,
};
pub use commands::{
    issue_attack_move_orders_to_selection, issue_attack_orders_to_selection,
    issue_idle_orders_to_selection, issue_move_orders_to_selection, MoveOrderUnitTrace,
    MoveOrdersReport,
};
pub use picking::{
    cursor_screen_position, cursor_world_ray, pick_unit_along_ray,
    pick_unit_command_target_along_ray, unit_pick_radius, world_position_to_screen,
};
pub use selection::SelectedUnits;