//! Vendor trade window (seller stock grid).

pub(crate) mod panel;

pub use panel::{
    TradePanelCloseButton, TradePanelRoot, handle_trade_panel_close, reconcile_trade_panel,
    spawn_trade_panel, sync_trade_panel_contents, sync_trade_panel_visibility,
};
