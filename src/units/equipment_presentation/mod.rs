//! Equipment visual presentation sync (Slice 7).

mod bones;
mod components;
mod morph_sync;
mod profile;
mod resolve;
mod skinned_overlay;
mod sync;

#[cfg(test)]
mod morph_sync_tests;
#[cfg(test)]
mod tests;

pub use components::UnitEquipmentVisual;
pub use morph_sync::sync_unit_equipment_morphs;
pub use skinned_overlay::finalize_skinned_equipment_overlays;
pub use sync::{UnitEquipmentPresentationIndex, sync_unit_equipment_presentation};
