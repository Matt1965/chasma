//! Initial authored work skill definitions.

use super::definition::WorkSkillDefinition;

/// Six initial work skills. Adding or removing entries here updates catalog-driven consumers.
pub fn starter_work_skill_definitions() -> Vec<WorkSkillDefinition> {
    vec![
        WorkSkillDefinition::new("farming", "Farming", 10),
        WorkSkillDefinition::new("general_labor", "General Labor", 20),
        WorkSkillDefinition::new("construction", "Construction", 30),
        WorkSkillDefinition::new("cooking", "Cooking", 40),
        WorkSkillDefinition::new("science", "Science", 50),
        WorkSkillDefinition::new("smithing", "Smithing", 60),
    ]
}
