//! Convert a starting squad draft into authoritative gameplay units (CG8).

use bevy::prelude::*;

use crate::world::{
    Affiliation, AppearanceProfileCatalog, InventoryCatalogCtx, OriginAppearanceSnapshot,
    OriginSpawnAnchor, OriginSquadMemberSnapshot, UnitCatalog, UnitOwnership, UnitSource,
    WorldData, apply_member_inventory_loadout, create_unit_with_ownership_and_appearance,
    ground_world_position, member_spawn_global_position, origin_member_formation_offsets,
    world_position_from_global,
};

use super::draft::StartingSquadDraft;

pub fn spawn_starting_squad_from_draft(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    anchor: &OriginSpawnAnchor,
    draft: &StartingSquadDraft,
) -> Result<Vec<crate::world::UnitId>, String> {
    let count = draft.members.len();
    if count == 0 {
        return Err("starting squad draft has no members".to_string());
    }
    let formation = origin_member_formation_offsets(count);
    let facing = Quat::from_rotation_y(anchor.yaw_deg.to_radians());
    let ownership = UnitOwnership::with_affiliation(Affiliation::Player);
    let layout = world.layout();
    let mut spawned = Vec::with_capacity(count);
    for (index, member) in draft.members.iter().enumerate() {
        let global = member_spawn_global_position(anchor, formation[index], member.preview_offset);
        let position = ground_world_position(world, world_position_from_global(global, layout))
            .unwrap_or_else(|| world_position_from_global(global, layout));
        let record = create_unit_with_ownership_and_appearance(
            unit_catalog,
            appearance_profiles,
            world,
            &member.definition_id,
            position,
            UnitSource::Authored,
            ownership.clone(),
            member.appearance.appearance.clone(),
            facing,
            Some(inventory_ctx),
        )
        .map_err(|error| format!("{error:?}"))?;
        let snapshot = OriginSquadMemberSnapshot {
            role_label: member.role_label.clone(),
            definition_id: member.definition_id.clone(),
            preview_offset_x: member.preview_offset.x,
            preview_offset_y: member.preview_offset.y,
            preview_offset_z: member.preview_offset.z,
            appearance: OriginAppearanceSnapshot::from_unit_appearance(
                &member.appearance.appearance,
            ),
            personal_inventory: member.personal_inventory.clone(),
            equipment_slots: member.equipment_slots.clone(),
        };
        apply_member_inventory_loadout(world, inventory_ctx, record.id, &snapshot)?;
        spawned.push(record.id);
    }
    Ok(spawned)
}
