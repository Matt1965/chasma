//! Vendor pricing derived from authored item base value (provisional gameplay constants).

/// Fraction of vendor sell price paid when the vendor buys from the player.
pub const VENDOR_PURCHASE_PRICE_MULTIPLIER: f64 = 0.5;

/// Price the player pays per unit when buying from a vendor.
pub fn vendor_sell_price_gold(base_value_gold: u32) -> u32 {
    base_value_gold
}

/// Price the vendor pays per unit when buying from the player.
pub fn vendor_buy_price_gold(base_value_gold: u32) -> u32 {
    ((base_value_gold as f64) * VENDOR_PURCHASE_PRICE_MULTIPLIER).floor() as u32
}

pub fn trade_line_total(unit_price_gold: u32, quantity: u32) -> u32 {
    unit_price_gold.saturating_mul(quantity)
}
