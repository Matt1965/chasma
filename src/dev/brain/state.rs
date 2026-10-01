//! Brain window UI state (dev-only).

use bevy::prelude::*;

use crate::world::{SettlementId, UnitId};

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct BrainWindowState {
    pub view: BrainView,
    pub selected_card_id: Option<String>,
    pub selected_need_id: Option<String>,
    /// Inspect a worker from settlement view without changing world selection.
    pub unit_inspect_override: Option<UnitId>,
    pub source_records_expanded: bool,
    last_unit_context: Option<UnitId>,
    last_settlement_context: Option<SettlementId>,
}

impl Default for BrainWindowState {
    fn default() -> Self {
        Self {
            view: BrainView::Unit,
            selected_card_id: None,
            selected_need_id: None,
            unit_inspect_override: None,
            source_records_expanded: false,
            last_unit_context: None,
            last_settlement_context: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrainView {
    #[default]
    Unit,
    Settlement,
}

impl BrainWindowState {
    pub fn on_context_changed(&mut self, unit: Option<UnitId>, settlement: Option<SettlementId>) {
        if self.last_unit_context != unit {
            self.selected_card_id = None;
            self.source_records_expanded = false;
            if self.unit_inspect_override.is_some() && unit != self.unit_inspect_override {
                self.unit_inspect_override = None;
            }
            self.last_unit_context = unit;
        }
        if self.last_settlement_context != settlement {
            self.selected_need_id = None;
            self.selected_card_id = None;
            self.source_records_expanded = false;
            self.last_settlement_context = settlement;
        }
    }

    pub fn open_unit_inspect(&mut self, unit_id: UnitId) {
        self.view = BrainView::Unit;
        self.unit_inspect_override = Some(unit_id);
        self.selected_card_id = None;
        self.source_records_expanded = false;
    }
}
