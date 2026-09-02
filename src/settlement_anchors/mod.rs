//! Runtime settlement anchor presentation (ADR-133).

mod components;
mod plugin;
mod spawn;
mod sync;

pub use components::SettlementAnchorRenderEntity;
pub use plugin::SettlementAnchorRuntimePlugin;
pub use spawn::SettlementAnchorRenderIndex;
pub use sync::{SettlementAnchorRuntimeSystems, sync_settlement_anchor_render_entities};
