//! Individual self-maintenance — hunger and eating (ADR-134).

mod food;
mod nutrition;
mod state;
mod step;

#[cfg(test)]
mod phase7_tests;

pub use nutrition::{
    HungerStage, NutritionProfile, UnitNutritionState, evaluate_hunger_stage, hunger_stage_label,
};
#[cfg(test)]
pub use food::{
    eat_one_from_inventory, find_nearest_settlement_edible, is_edible_food, select_food_source,
};
#[cfg(test)]
pub use nutrition::{apply_nutrition_decay, restore_nutrition};
pub use state::{FoodSourceRef, SelfMaintenanceActivity, UnitSelfMaintenanceState};
pub use step::{
    SelfMaintenanceContext, hunger_prevents_work_claim, initialize_unit_nutrition,
    step_unit_nutrition_decay, step_unit_self_maintenance_post_movement,
    step_unit_self_maintenance_pre_work, unit_in_active_combat,
};
