//! Starter ResponseDefinitions — catalog-driven options for SA2 needs (SA3).
//!
//! No response performs work. Building/operation ids are authored capability tags only.

use super::definition::{
    CapabilityRequirement, ExpectedEffect, ResponseDefinition, ResponseType,
};
use crate::world::settlement::needs::NeedId;

pub fn starter_response_definitions() -> Vec<ResponseDefinition> {
    vec![
        // Food — multiple catalog paths, never a single hardcoded farm branch.
        ResponseDefinition::new(
            "increase_food_production",
            "Increase Food Production",
            "Enable or intensify operations that output food items.",
            [NeedId::new("food")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.8, 20.0),
            10,
            [
                CapabilityRequirement::SupportingOperation("grow_prispods".into()),
                CapabilityRequirement::AutomationEnabled,
            ],
        )
        .with_ai_tags(["production", "food"]),
        ResponseDefinition::new(
            "bake_bread_production",
            "Bake Bread",
            "Increase bakery bread output to relieve food pressure.",
            [NeedId::new("food")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.7, 25.0),
            5,
            [
                CapabilityRequirement::SupportingOperation("bake_bread".into()),
                CapabilityRequirement::AutomationEnabled,
            ],
        )
        .with_ai_tags(["production", "food"]),
        ResponseDefinition::new(
            "trade_for_food",
            "Trade for Food",
            "Acquire food via trade (stub availability until trade runtime exists).",
            [NeedId::new("food")],
            ResponseType::Trade,
            ExpectedEffect::new(0.5, 40.0),
            0,
            [CapabilityRequirement::Always],
        )
        .with_ai_tags(["trade", "food"]),
        ResponseDefinition::new(
            "construct_food_building",
            "Construct Food Building",
            "Construct a building that can support food operations.",
            [NeedId::new("food"), NeedId::new("construction")],
            ResponseType::ConstructBuilding,
            ExpectedEffect::new(0.6, 60.0),
            0,
            [CapabilityRequirement::Always],
        )
        .with_ai_tags(["construction", "food"]),
        // Construction — production path for materials (capability = operation, not building name).
        ResponseDefinition::new(
            "increase_construction_materials",
            "Increase Construction Materials",
            "Enable stone-extraction operations that supply construction.",
            [NeedId::new("construction")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.75, 25.0),
            12,
            [
                CapabilityRequirement::SupportingOperation("mine_stone".into()),
                CapabilityRequirement::AutomationEnabled,
            ],
        )
        .with_ai_tags(["production", "construction"]),
        ResponseDefinition::new(
            "advance_construction",
            "Advance Construction",
            "Prioritize incomplete construction sites.",
            [NeedId::new("construction")],
            ResponseType::ConstructBuilding,
            ExpectedEffect::new(0.9, 30.0),
            15,
            [CapabilityRequirement::Always],
        )
        .with_ai_tags(["construction"]),
        // Housing
        ResponseDefinition::new(
            "construct_housing",
            "Construct Housing",
            "Build additional residential capacity.",
            [NeedId::new("housing")],
            ResponseType::ConstructBuilding,
            ExpectedEffect::new(0.85, 50.0),
            5,
            [CapabilityRequirement::Always],
        )
        .with_ai_tags(["construction", "housing"]),
        // Defense
        ResponseDefinition::new(
            "defend_settlement",
            "Defend Settlement",
            "Adopt a defensive posture and prioritize defense buildings.",
            [NeedId::new("defense")],
            ResponseType::Defend,
            ExpectedEffect::new(0.75, 35.0),
            20,
            [CapabilityRequirement::MinAggression(1)],
        )
        .with_ai_tags(["defense"]),
        // Emergency-only defend option (unlocked by active_attack EmergencyDefinition).
        ResponseDefinition::new(
            "defend_settlement_emergency",
            "Emergency Defense",
            "Urgent defensive response unlocked during Active Attack emergencies.",
            [NeedId::new("defense")],
            ResponseType::Defend,
            ExpectedEffect::new(0.9, 20.0),
            40,
            [CapabilityRequirement::Always],
        )
        .with_ai_tags(["defense", "emergency_only"]),
        ResponseDefinition::new(
            "construct_defenses",
            "Construct Defenses",
            "Construct walls/towers/barracks-class buildings.",
            [NeedId::new("defense"), NeedId::new("construction")],
            ResponseType::ConstructBuilding,
            ExpectedEffect::new(0.7, 55.0),
            10,
            [CapabilityRequirement::Always],
        )
        .with_ai_tags(["defense", "construction"]),
        // Research
        ResponseDefinition::new(
            "pursue_research",
            "Pursue Research",
            "Run research-capable operations.",
            [NeedId::new("research")],
            ResponseType::Research,
            ExpectedEffect::new(0.8, 25.0),
            0,
            [
                CapabilityRequirement::SupportingOperation("research".into()),
                CapabilityRequirement::AutomationEnabled,
            ],
        )
        .with_ai_tags(["research"]),
        // Expansion / Growth
        ResponseDefinition::new(
            "expand_settlement",
            "Expand Settlement",
            "Grow footprint when expansion policy allows.",
            [NeedId::new("expansion")],
            ResponseType::Expand,
            ExpectedEffect::new(0.6, 45.0),
            -5,
            [CapabilityRequirement::ExpansionEnabled],
        )
        .with_ai_tags(["expansion", "growth"]),
        // Luxury
        ResponseDefinition::new(
            "increase_luxury_production",
            "Increase Luxury Production",
            "Intensify luxury-oriented production (iron bars as luxury proxy).",
            [NeedId::new("luxury")],
            ResponseType::IncreaseProduction,
            ExpectedEffect::new(0.55, 30.0),
            -10,
            [
                CapabilityRequirement::SupportingOperation("smelt_iron".into()),
                CapabilityRequirement::AutomationEnabled,
            ],
        )
        .with_ai_tags(["production", "luxury"]),
        // Generic decrease / repair / recruit stubs (catalog completeness)
        ResponseDefinition::new(
            "decrease_luxury_production",
            "Decrease Luxury Production",
            "Throttle luxury production when other needs dominate (option only).",
            [NeedId::new("luxury")],
            ResponseType::DecreaseProduction,
            ExpectedEffect::new(0.2, 5.0),
            -20,
            [CapabilityRequirement::SupportingOperation("smelt_iron".into())],
        )
        .with_ai_tags(["production", "luxury"]),
        ResponseDefinition::new(
            "repair_buildings",
            "Repair Buildings",
            "Repair damaged buildings (availability stub).",
            [NeedId::new("housing"), NeedId::new("defense")],
            ResponseType::RepairBuilding,
            ExpectedEffect::new(0.4, 20.0),
            0,
            [CapabilityRequirement::Always],
        )
        .with_ai_tags(["repair"]),
        ResponseDefinition::new(
            "recruit_workers",
            "Recruit Workers",
            "Recruit additional workers (availability stub).",
            [NeedId::new("expansion"), NeedId::new("housing")],
            ResponseType::Recruit,
            ExpectedEffect::new(0.35, 40.0),
            0,
            [CapabilityRequirement::Always],
        )
        .with_ai_tags(["recruit"]),
    ]
}
