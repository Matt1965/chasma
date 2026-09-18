//! Continuous origin/squad character generation stage (CG7/CG8).

mod actions;
mod camera;
mod focus;
mod plugin;
mod preview;
mod preview_spawn;
mod screen;
mod session;

#[cfg(test)]
mod tests;

pub use plugin::OriginSelectPlugin;
pub use session::StartingSquadSession;
