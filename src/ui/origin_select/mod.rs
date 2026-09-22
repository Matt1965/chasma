//! Continuous origin/squad character generation stage (CG7/CG8).

mod actions;
mod camera;
mod focus;
mod plugin;
mod presentation;
mod preview;
#[cfg(test)]
mod preview_invariants;
mod preview_reconcile;
mod preview_spawn;
mod screen;
mod session;

#[cfg(test)]
mod preview_lifecycle_tests;
#[cfg(test)]
mod tests;

pub use plugin::OriginSelectPlugin;
pub use session::StartingSquadSession;
