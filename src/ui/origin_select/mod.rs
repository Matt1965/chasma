//! Starting-origin selection and multi-actor preview (CG7).

mod actions;
mod camera;
mod plugin;
mod preview;
mod screen;
mod session;

#[cfg(test)]
mod tests;

pub use plugin::OriginSelectPlugin;
pub use session::OriginSelectSession;
