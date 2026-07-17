//! Starter field response profiles for tests and dev fallback (ADR-104 TF4).

use super::definition::{
    FieldResponsePoint, FieldResponseProfileDefinition, efficiency_basis_points_from_percent,
    field_value_from_percent,
};
use super::id::FieldResponseProfileId;

pub fn starter_profiles() -> Vec<FieldResponseProfileDefinition> {
    vec![
        iron_mine_monotonic(),
        copper_mine_monotonic(),
        stone_quarry_monotonic(),
        water_crop_preferred_range(),
        water_well_monotonic(),
    ]
}

fn iron_mine_monotonic() -> FieldResponseProfileDefinition {
    FieldResponseProfileDefinition::from_points(
        FieldResponseProfileId::new("iron_mine_monotonic"),
        "Iron Mine Monotonic",
        vec![
            point_percent(0.0, 0.0),
            point_percent(20.0, 0.0),
            point_percent(50.0, 50.0),
            point_percent(80.0, 100.0),
            point_percent(100.0, 120.0),
        ],
    )
    .expect("iron mine profile")
    .with_description("Monotonic iron extraction response with bonus above rich deposits.")
}

fn copper_mine_monotonic() -> FieldResponseProfileDefinition {
    FieldResponseProfileDefinition::from_points(
        FieldResponseProfileId::new("copper_mine_monotonic"),
        "Copper Mine Monotonic",
        vec![
            point_percent(0.0, 0.0),
            point_percent(25.0, 0.0),
            point_percent(55.0, 55.0),
            point_percent(85.0, 100.0),
            point_percent(100.0, 115.0),
        ],
    )
    .expect("copper mine profile")
    .with_description("Monotonic copper extraction response.")
}

fn stone_quarry_monotonic() -> FieldResponseProfileDefinition {
    FieldResponseProfileDefinition::from_points(
        FieldResponseProfileId::new("stone_quarry_monotonic"),
        "Stone Quarry Monotonic",
        vec![
            point_percent(0.0, 0.0),
            point_percent(15.0, 0.0),
            point_percent(40.0, 45.0),
            point_percent(75.0, 100.0),
            point_percent(100.0, 110.0),
        ],
    )
    .expect("stone quarry profile")
    .with_description("Monotonic stone quarry response.")
}

fn water_crop_preferred_range() -> FieldResponseProfileDefinition {
    FieldResponseProfileDefinition::from_points(
        FieldResponseProfileId::new("water_crop_preferred_range"),
        "Water Crop Preferred Range",
        vec![
            point_percent(0.0, 0.0),
            point_percent(20.0, 40.0),
            point_percent(45.0, 100.0),
            point_percent(70.0, 100.0),
            point_percent(90.0, 30.0),
            point_percent(100.0, 0.0),
        ],
    )
    .expect("water crop profile")
    .with_description("Crop yield peaks in a moderate water band and penalizes excess moisture.")
}

fn water_well_monotonic() -> FieldResponseProfileDefinition {
    FieldResponseProfileDefinition::from_points(
        FieldResponseProfileId::new("water_well_monotonic"),
        "Water Well Monotonic",
        vec![
            point_percent(0.0, 0.0),
            point_percent(10.0, 0.0),
            point_percent(35.0, 35.0),
            point_percent(70.0, 100.0),
            point_percent(100.0, 100.0),
        ],
    )
    .expect("water well profile")
    .with_description("Well output rises with local water potential up to saturation.")
}

fn point_percent(field_percent: f32, efficiency_percent: f32) -> FieldResponsePoint {
    FieldResponsePoint {
        field_value: field_value_from_percent(field_percent),
        efficiency_basis_points: efficiency_basis_points_from_percent(efficiency_percent),
    }
}
