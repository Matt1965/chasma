//! Client-local Catalog session state (Slice 4).

use bevy::prelude::*;

use super::super::dev_mode::DevTab;

/// Catalog-specific session fields (client-local, not scene-persisted).

#[derive(Debug, Clone, PartialEq)]

pub struct CatalogSessionState {
    pub last_tab: DevTab,

    /// Compact contextual status (placement, scene, errors).
    pub status_message: String,

    /// Frames remaining before clearing non-error status (0 = persistent error).
    pub status_ttl_frames: u32,
}

impl Default for CatalogSessionState {
    fn default() -> Self {
        Self {
            last_tab: DevTab::Units,

            status_message: String::new(),

            status_ttl_frames: 0,
        }
    }
}

impl CatalogSessionState {
    pub fn set_status(&mut self, message: impl Into<String>, ttl_frames: u32) {
        self.status_message = message.into();

        self.status_ttl_frames = ttl_frames;
    }

    pub fn clear_status(&mut self) {
        self.status_message.clear();

        self.status_ttl_frames = 0;
    }

    pub fn tick_status_ttl(&mut self) {
        if self.status_ttl_frames > 0 {
            self.status_ttl_frames -= 1;

            if self.status_ttl_frames == 0 {
                self.status_message.clear();
            }
        }
    }
}

pub fn is_catalog_tab(tab: DevTab) -> bool {
    matches!(
        tab,
        DevTab::Units | DevTab::Doodads | DevTab::Buildings | DevTab::Items
    )
}

/// Asset tabs that expose contextual placement controls (not Items).

pub fn is_placement_catalog_tab(tab: DevTab) -> bool {
    matches!(tab, DevTab::Units | DevTab::Doodads | DevTab::Buildings)
}

/// Tabs rendered in the Catalog title bar.

pub fn visible_tabs() -> &'static [DevTab] {
    &[
        DevTab::Units,
        DevTab::Doodads,
        DevTab::Buildings,
        DevTab::Items,
    ]
}

pub fn tab_is_visible(tab: DevTab) -> bool {
    visible_tabs().contains(&tab)
}

pub fn on_tab_selected(session: &mut CatalogSessionState, tab: DevTab) {
    if is_catalog_tab(tab) {
        session.last_tab = tab;
    }
}

pub fn next_visible_tab(current: DevTab) -> DevTab {
    let tabs = visible_tabs();

    let pos = tabs.iter().position(|&tab| tab == current).unwrap_or(0);

    tabs[(pos + 1) % tabs.len()]
}
