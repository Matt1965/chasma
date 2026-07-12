//! Unit overhead health bars — read-only vitals presentation (ADR-062 C9).

mod sync;
mod visibility;

pub use sync::{
    UnitHealthBar, UnitHealthBarState, billboard_unit_health_bars, health_bar_color,
    sync_unit_health_bars,
};
pub use visibility::{health_percent, should_show_health_bar};
