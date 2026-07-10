//! Disposable ECS mirrors for authoritative projectile simulation (ADR-060 C7).

mod assets;
mod components;
mod plugin;
mod spawn;
mod sync;

pub use assets::ProjectileSceneAssets;
pub use components::{ProjectileRenderEntity, ProjectileSceneRoot};
pub use plugin::ProjectilesRuntimePlugin;
pub use spawn::ProjectileRenderIndex;
pub use sync::{ProjectileRuntimeSystems, ProjectileSyncOverrides, sync_projectile_render_entities};
