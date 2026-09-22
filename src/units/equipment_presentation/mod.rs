//! Equipment visual presentation sync (Slice 7).

mod bones;
mod components;
mod corpse_sync;
mod morph_sync;
mod profile;
mod resolve;
mod skinned_overlay;
mod sync;

#[cfg(test)]
mod morph_sync_tests;
#[cfg(test)]
mod tests;

pub use components::{CorpseEquipmentVisual, UnitEquipmentVisual};
pub use corpse_sync::{
    CorpseEquipmentPresentationIndex, sync_corpse_equipment_presentation,
};
pub use morph_sync::sync_unit_equipment_morphs;
pub use resolve::EquipmentPresentationOwner;
pub use skinned_overlay::finalize_skinned_equipment_overlays;
pub use sync::{UnitEquipmentPresentationIndex, sync_unit_equipment_presentation};
