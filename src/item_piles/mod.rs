//! Runtime item pile presentation (ADR-090 I4).

mod components;
mod plugin;
mod sync;

pub use components::ItemPileRenderEntity;
pub use plugin::ItemPileRuntimePlugin;
pub use sync::{ItemPileRuntimeSystems, sync_item_pile_render_entities};
