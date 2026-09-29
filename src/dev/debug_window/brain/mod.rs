//! Brain diagnostics — dev-only "why is this unit doing that?" tooling.

mod history;
mod model;
mod panel;

#[cfg(test)]
mod tests;

pub use history::{BrainDecisionHistory, tick_brain_decision_history};
pub use panel::{BrainPanelState, handle_brain_view_buttons, setup_brain_panel, sync_brain_panel};
