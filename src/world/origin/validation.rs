use crate::world::{AppearanceProfileCatalog, ItemCatalog, UnitCatalog};

use super::definition::OriginDefinition;
use super::snapshot::validate_member_inventory_snapshots;

pub fn validate_origin_definition(
    origin: &OriginDefinition,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    item_catalog: &ItemCatalog,
) -> Result<(), String> {
    if origin.members.is_empty() {
        return Err(format!(
            "origin `{}` must include at least one member",
            origin.id.as_str()
        ));
    }
    for member in &origin.members {
        unit_catalog
            .get(&member.definition_id)
            .ok_or_else(|| {
                format!(
                    "origin `{}` references missing unit `{}`",
                    origin.id.as_str(),
                    member.definition_id.as_str()
                )
            })?;
        if member.appearance.profile_id.is_empty() {
            return Err(format!(
                "origin `{}` member `{}` is missing appearance profile",
                origin.id.as_str(),
                member.role_label
            ));
        }
        if appearance_profiles
            .get(&crate::world::unit::appearance::AppearanceProfileId::new(
                &member.appearance.profile_id,
            ))
            .is_none()
        {
            return Err(format!(
                "origin `{}` member `{}` references missing appearance profile `{}`",
                origin.id.as_str(),
                member.role_label,
                member.appearance.profile_id
            ));
        }
        validate_member_inventory_snapshots(member, item_catalog)?;
    }
    Ok(())
}
