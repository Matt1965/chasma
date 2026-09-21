//! Starting-origin definitions for New Game / squad preview (CG7).

mod capture;
mod catalog;
mod definition;
mod id;
mod persistence;
mod restore;
mod spawn;
mod snapshot;
mod starter;
mod validation;

#[cfg(test)]
mod tests;

pub use capture::{
    capture_member_inventory_loadout, capture_origin_member_from_unit,
    capture_squad_members_from_selection, ordered_selected_unit_ids, preview_offset_for_index,
};
pub use catalog::OriginCatalog;
pub use definition::OriginDefinition;
pub use id::OriginId;
pub use persistence::{
    ORIGINS_RON_PATH, load_dev_origin_catalog, load_origins_from_ron, save_origins_to_ron,
};
pub use restore::apply_member_inventory_loadout;
pub use spawn::{
    OriginSpawnAnchor, member_spawn_global_position, origin_member_formation_offsets,
    world_position_from_global,
};
pub use snapshot::{
    OriginAppearanceSnapshot, OriginEquipmentSlotSnapshot, OriginSquadMemberSnapshot,
};
pub use starter::{seed_origin_catalog, seed_origin_definitions};
pub use validation::validate_origin_definition;
