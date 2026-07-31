//! Catalog tab labels (Slice 4).

use super::super::dev_mode::DevTab;

pub fn tab_label(tab: DevTab) -> &'static str {
    match tab {
        DevTab::Units => "Units",

        DevTab::Doodads => "Doodads",

        DevTab::Buildings => "Buildings",

        DevTab::Items => "Items",

        DevTab::Scenes => "Scenes",

        DevTab::Debug => "Debug",

        DevTab::WorldTools => "World",

        DevTab::TerrainFields => "Fields",
    }
}
