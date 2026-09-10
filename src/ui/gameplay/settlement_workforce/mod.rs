//! Settlement Workforce screen module.

mod content;
mod input;
mod layout;
mod panel;
mod scroll;
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
pub use layout::{
    CLOSE_BUTTON_LABEL, MATRIX_COLUMN_COUNT, PANEL_MIN_WIDTH_PX, PANEL_WIDTH_PX,
    forbidden_workforce_ui_characters, matrix_min_width, permission_checkbox_label,
};
pub use panel::{
    SettlementWorkforceMatrixBody, SettlementWorkforceMatrixContentHost,
    SettlementWorkforceMatrixDataRow, SettlementWorkforceMatrixHeaderHost,
    SettlementWorkforceMatrixHeaderRow, SettlementWorkforceMatrixHorizontalScroll,
    SettlementWorkforceMatrixRowsScroll, SettlementWorkforcePanelCloseButton,
    SettlementWorkforcePanelRoot, SettlementWorkforcePanelTitleText,
    SettlementWorkforceVerticalScrollbar, SettlementWorkforceVerticalScrollbarThumb,
    WorkforceAllowAllButton, WorkforceClearAllButton, WorkforcePermissionCheckbox,
    handle_settlement_workforce_close_button, handle_settlement_workforce_controls,
    spawn_settlement_workforce_panel, sync_settlement_workforce_panel,
    sync_settlement_workforce_panel_dimensions, sync_settlement_workforce_panel_visibility,
};
pub use scroll::{
    SettlementWorkforceScrollPlugin, SettlementWorkforceScrollState,
    WORKFORCE_SCROLL_WHEEL_LINE_PX, WORKFORCE_SCROLLBAR_MIN_THUMB_PX,
    WORKFORCE_SCROLLBAR_TRACK_WIDTH_PX, clamp_scroll_offset_y, max_scroll_y,
    reset_settlement_workforce_scroll, scrollbar_thumb_metrics,
};
pub use state::SettlementWorkforcePanelState;
