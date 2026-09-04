//! Stable dev-window identities (Slice 3).

/// Client-local dev window identity.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]

pub enum DevWindowId {
    /// WorldData snapshot save/load (client-local scenes).
    Save,

    /// Asset discovery and placement.
    Catalog,

    /// World-object inspection driven by shared selection (Slice 5).
    SelectedObject,

    /// Building navigation blueprint authoring (Slice 7).
    NavigationEditor,

    /// Runtime diagnostic overlays (Slice 8).
    Debug,

    /// Environment and time-of-day authoring (Slice 8).
    World,

    /// Camera-focused settlement dev tools.
    Settlement,

    /// Terrain field construction and probe (Slice 8).
    Fields,
}

impl DevWindowId {
    pub fn title(self) -> &'static str {
        match self {
            Self::Save => "Save",

            Self::Catalog => "Catalog",

            Self::SelectedObject => "Selected Object",

            Self::NavigationEditor => "Navigation Editor",

            Self::Debug => "Debug",

            Self::World => "World",

            Self::Settlement => "Settlement",

            Self::Fields => "Fields",
        }
    }

    /// Label shown on the workspace launcher button (may differ from title bar).

    pub fn launcher_label(self) -> &'static str {
        self.title()
    }

    pub fn default_visible(self) -> bool {
        match self {
            Self::Catalog | Self::SelectedObject => true,

            Self::Save
            | Self::NavigationEditor
            | Self::Debug
            | Self::World
            | Self::Settlement
            | Self::Fields => false,
        }
    }

    pub fn supports_collapse(self) -> bool {
        match self {
            Self::Save
            | Self::Catalog
            | Self::SelectedObject
            | Self::NavigationEditor
            | Self::Debug
            | Self::World
            | Self::Settlement
            | Self::Fields => true,
        }
    }

    /// Windows shown in the primary launcher row.

    pub const WINDOWS_LAUNCHER: &'static [DevWindowId] =
        &[Self::Save, Self::Catalog, Self::SelectedObject];

    /// Windows shown in the Advanced launcher row.

    pub const ADVANCED_LAUNCHER: &'static [DevWindowId] = &[
        Self::Debug,
        Self::World,
        Self::Settlement,
        Self::Fields,
        Self::NavigationEditor,
    ];
}
