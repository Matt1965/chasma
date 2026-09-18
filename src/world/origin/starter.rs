use bevy::prelude::*;

use crate::world::UnitDefinitionId;

use super::catalog::OriginCatalog;
use super::definition::{OriginDefinition, OriginRosterMember};
use super::id::OriginId;

/// Starter origins for CG7 preview/selection until workbook import exists.
pub fn starter_origin_definitions() -> Vec<OriginDefinition> {
    vec![
        OriginDefinition {
            id: OriginId::new("exile_pair"),
            display_name: "Exile Pair".to_string(),
            description: "Two outcasts with nothing but each other.".to_string(),
            roster: vec![
                OriginRosterMember {
                    role_label: "Leader".to_string(),
                    definition_id: UnitDefinitionId::new("U-0004"),
                    preview_offset: Vec3::new(-0.75, 0.0, 0.0),
                },
                OriginRosterMember {
                    role_label: "Companion".to_string(),
                    definition_id: UnitDefinitionId::new("U-0005"),
                    preview_offset: Vec3::new(0.75, 0.0, 0.0),
                },
            ],
        },
        OriginDefinition {
            id: OriginId::new("lone_survivor"),
            display_name: "Lone Survivor".to_string(),
            description: "One survivor against the chasm.".to_string(),
            roster: vec![OriginRosterMember {
                role_label: "Survivor".to_string(),
                definition_id: UnitDefinitionId::new("U-0004"),
                preview_offset: Vec3::ZERO,
            }],
        },
    ]
}

pub fn starter_origin_catalog() -> OriginCatalog {
    OriginCatalog::from_definitions(starter_origin_definitions()).expect("starter origins")
}
