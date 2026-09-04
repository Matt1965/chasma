//! Player-facing Unit Skills screen (U hotkey).

mod content;
mod input;
mod panel;
mod state;

#[cfg(test)]
mod tests;

pub use content::{
    UnitSkillsPanelSnapshot, UnitSkillsSection, UnitSkillsStatLine, build_unit_skills_snapshot,
    format_unit_skills_panel_text, panel_contains_workforce_permission_controls,
};
pub use input::collect_unit_skills_keyboard_input;
pub use panel::{
    UnitSkillsPanelCloseButton, UnitSkillsPanelRoot, handle_unit_skills_close_button,
    reconcile_unit_skills_panel, spawn_unit_skills_panel, sync_unit_skills_panel,
    sync_unit_skills_panel_visibility,
};
pub use state::UnitSkillsPanelState;
