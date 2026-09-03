//! Player-facing building shell text (BP1).

use crate::world::BuildingLifecycleState;

pub fn format_building_shell(
    display_name: &str,
    lifecycle: BuildingLifecycleState,
    current_hp: u32,
    max_hp: u32,
) -> String {
    format!(
        "{display_name}\n{}\nHP {current_hp} / {max_hp}",
        lifecycle.label()
    )
}

/// Single-line header for the owned building menu (BP2).
pub fn format_building_header_line(
    display_name: &str,
    lifecycle_label: &str,
    current_hp: u32,
    max_hp: u32,
) -> String {
    format!("{display_name}\n{lifecycle_label} | HP {current_hp}/{max_hp}")
}
