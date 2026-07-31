//! Shared dev UI tooltips (Slice 7 foundation; full widget rollout Slice 9).

mod components;
mod state;
mod systems;

pub use components::{DevTooltipContent, DevTooltipHoverZone, DevTooltipTarget};
pub use state::{DevTooltipState, TOOLTIP_HOVER_DELAY_SECS};
pub use systems::{dismiss_dev_tooltip, setup_dev_tooltip, sync_dev_tooltip_presentation};
