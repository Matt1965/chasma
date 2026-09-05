//! Settlement Workforce screen module.

mod content;
mod input;
mod panel;
mod state;

#[cfg(test)]
mod tests;

pub use content::{
    NO_FOCUSED_SETTLEMENT_MESSAGE, NO_SETTLEMENT_WORKERS_MESSAGE, SettlementWorkforceSnapshot,
    WorkforceMatrixCell, WorkforceMatrixRow, build_settlement_workforce_snapshot,
    permission_column_labels, settlement_workforce_member_unit_ids,
    snapshot_contains_permission_column,
};
pub use input::collect_settlement_workforce_keyboard_input;
pub use panel::{
    SettlementWorkforcePanelCloseButton, SettlementWorkforcePanelRoot,
    SettlementWorkforcePanelTitleText, WorkforceAllowAllButton, WorkforceClearAllButton,
    WorkforcePermissionCheckbox, handle_settlement_workforce_close_button,
    handle_settlement_workforce_controls, spawn_settlement_workforce_panel,
    sync_settlement_workforce_panel, sync_settlement_workforce_panel_visibility,
};
pub use state::SettlementWorkforcePanelState;
