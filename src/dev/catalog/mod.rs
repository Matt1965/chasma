//! Catalog window — tab routing and contextual placement (Slice 4).

mod components;

mod panel_sync;

mod placement_controls;

mod state;

mod tabs;

pub(crate) use components::{
    DevCatalogStatusText, DevContextualPlacementAction, DevContextualPlacementButton,
    DevContextualPlacementSection, DevContextualPlacementTitle, DevPlacementActiveBanner,
    DevTabChrome,
};

pub use panel_sync::{
    all_catalog_tabs, spawn_tab_label, sync_dev_catalog_chrome, track_catalog_tab_selection,
};

pub use placement_controls::{
    PlacementControlField, PlacementControlSet, PlacementUiContext, placement_control_set,
    placement_control_tooltip, placement_status_line,
};

pub use state::{
    CatalogSessionState, is_catalog_tab, is_placement_catalog_tab, next_visible_tab,
    on_tab_selected, tab_is_visible, visible_tabs,
};

pub use tabs::tab_label;

#[cfg(test)]
mod tests;
