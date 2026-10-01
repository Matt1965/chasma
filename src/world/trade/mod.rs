//! Vendor trade pricing and authoritative buy/sell execution.

mod pricing;
mod transaction;

pub use pricing::{
    VENDOR_PURCHASE_PRICE_MULTIPLIER, trade_line_total, vendor_buy_price_gold,
    vendor_sell_price_gold,
};
pub use transaction::{
    TradeDirection, TradeTransactionError, TradeTransferMode, TradeTransferOutcome,
    TradeTransferRequest, classify_trade_transfer, execute_trade_transfer,
};

#[cfg(test)]
mod tests;
