//! Brain diagnostics — dedicated dev window for unit/settlement AI provenance.

mod document;
mod history;
mod model;
mod state;
mod window;

#[cfg(test)]
mod tests;

pub use history::{BrainDecisionHistory, tick_brain_decision_history};
pub use state::BrainWindowState;
pub use window::{handle_brain_window_input, setup_brain_window_panel, sync_brain_window};
