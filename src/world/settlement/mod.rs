//! Settlement treasuries and physical gold deposits (ADR-093 I7).

mod access;
mod authoring;
mod deposit;
mod error;
mod id;
mod record;
mod store;

pub use access::{
    TreasuryAccessPolicy, TreasuryAccessResult, building_supports_settlement_treasury,
    can_unit_deposit_to_treasury, settlement_interaction_position, settlement_interaction_space,
};
pub use authoring::{CreateSettlementReport, create_settlement_with_treasury};
pub use deposit::{DepositGoldReport, deposit_gold};
pub use error::TreasuryError;
pub use id::{SettlementId, TreasuryId};
pub use record::{
    SettlementOwnership, SettlementRecord, SettlementTreasuryRecord, TreasuryTransactionRecord,
};
pub use store::SettlementStore;

#[cfg(test)]
mod tests;
