use bevy::prelude::Vec3;

use crate::world::{
    AppearanceProfileCatalog, UnitCatalog, UnitDefinitionId, resolve_canonical_default_appearance,
};

use super::catalog::OriginCatalog;
use super::definition::OriginDefinition;
use super::id::OriginId;
use super::snapshot::{OriginAppearanceSnapshot, OriginSquadMemberSnapshot};

fn member_from_definition(
    role_label: &str,
    definition_id: &str,
    preview_offset: Vec3,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
) -> OriginSquadMemberSnapshot {
    let definition_id = UnitDefinitionId::new(definition_id);
    let definition = unit_catalog.get(&definition_id).expect("starter unit definition");
    let appearance = resolve_canonical_default_appearance(definition, appearance_profiles)
        .expect("starter appearance");
    OriginSquadMemberSnapshot {
        role_label: role_label.to_string(),
        definition_id,
        preview_offset_x: preview_offset.x,
        preview_offset_y: preview_offset.y,
        preview_offset_z: preview_offset.z,
        appearance: OriginAppearanceSnapshot::from_unit_appearance(&appearance),
        personal_inventory: None,
        equipment_slots: Vec::new(),
    }
}

pub fn seed_origin_definitions(
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
) -> Vec<OriginDefinition> {
    vec![
        OriginDefinition {
            id: OriginId::new("exile_pair"),
            display_name: "Exile Pair".to_string(),
            description: "Two outcasts with nothing but each other.".to_string(),
            members: vec![
                member_from_definition(
                    "Leader",
                    "U-0004",
                    Vec3::new(-0.75, 0.0, 0.0),
                    unit_catalog,
                    appearance_profiles,
                ),
                member_from_definition(
                    "Companion",
                    "U-0005",
                    Vec3::new(0.75, 0.0, 0.0),
                    unit_catalog,
                    appearance_profiles,
                ),
            ],
            start_x: 272.0,
            start_z: 105.0,
            yaw_deg: 0.0,
        },
        OriginDefinition {
            id: OriginId::new("lone_survivor"),
            display_name: "Lone Survivor".to_string(),
            description: "One survivor against the chasm.".to_string(),
            members: vec![member_from_definition(
                "Survivor",
                "U-0004",
                Vec3::ZERO,
                unit_catalog,
                appearance_profiles,
            )],
            start_x: 274.0,
            start_z: 103.0,
            yaw_deg: 0.0,
        },
    ]
}

pub fn seed_origin_catalog(
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
) -> OriginCatalog {
    OriginCatalog::from_definitions(seed_origin_definitions(unit_catalog, appearance_profiles))
        .expect("seed origins")
}
