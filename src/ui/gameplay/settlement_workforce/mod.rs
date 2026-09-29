//! Settlement Workforce screen module.

mod content;
mod input;
mod layout;
mod panel;
mod scroll;
mod state;

#[cfg(test)]
mod tests;

pub use content::build_settlement_workforce_snapshot;
#[cfg(test)]
pub use content::snapshot_contains_permission_column;
#[cfg(test)]
pub use layout::{
    CLOSE_BUTTON_LABEL, MATRIX_COLUMN_COUNT, PANEL_MIN_WIDTH_PX, PANEL_WIDTH_PX,
    forbidden_workforce_ui_characters, matrix_min_width, permission_checkbox_label,
};
#[cfg(test)]
pub use panel::{
    SettlementWorkforceMatrixBody, SettlementWorkforceMatrixContentHost,
    SettlementWorkforceMatrixDataRow, SettlementWorkforceMatrixHeaderHost,
    SettlementWorkforceMatrixHeaderRow, SettlementWorkforceMatrixHorizontalScroll,
    SettlementWorkforceMatrixRowsScroll, SettlementWorkforcePanelCloseButton,
    SettlementWorkforcePanelRoot, SettlementWorkforcePanelTitleText,
    SettlementWorkforceVerticalScrollbar, WorkforceAllowAllButton, WorkforcePermissionCheckbox,
};
#[cfg(test)]
pub use scroll::{
    SettlementWorkforceScrollState, clamp_scroll_offset_y, max_scroll_y,
};
pub use input::collect_settlement_workforce_keyboard_input;
pub use panel::{
    handle_settlement_workforce_close_button, handle_settlement_workforce_controls,
    spawn_settlement_workforce_panel, sync_settlement_workforce_panel,
    sync_settlement_workforce_panel_dimensions, sync_settlement_workforce_panel_visibility,
};
pub use scroll::SettlementWorkforceScrollPlugin;
pub use state::SettlementWorkforcePanelState;

#[cfg(test)]
pub use content::{
    NO_FOCUSED_SETTLEMENT_MESSAGE, NO_SETTLEMENT_WORKERS_MESSAGE, permission_column_labels,
    settlement_workforce_member_unit_ids,
};
