//! Catalog window — tab routing and contextual placement (Slice 4).

mod components;

mod panel_sync;

mod placement_controls;

pub(crate) mod scroll;

mod state;

mod tabs;

pub(crate) use components::{
    DevCatalogStatusText, DevContextualPlacementAction, DevContextualPlacementButton,
    DevContextualPlacementSection, DevContextualPlacementTitle, DevPlacementActiveBanner,
    DevTabChrome,
};

pub use panel_sync::{
    all_catalog_tabs, spawn_tab_label, sync_catalog_placement_button_labels,
    sync_dev_catalog_chrome, track_catalog_tab_selection,
};

pub use placement_controls::{
    PlacementControlField,
    placement_control_tooltip,
};

pub use scroll::{
    CatalogScrollMetrics, ROW_HEIGHT_PX, catalog_row_pool_capacity, clamp_scroll_offset,
    visible_row_count,
};
pub use state::{
    CatalogSessionState, next_visible_tab,
};


#[cfg(test)]
mod tests;
