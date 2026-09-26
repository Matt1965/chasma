//! Dialogue / social interaction authority (relationship-gated eligibility).

mod action;
mod config;
mod content;
mod eligibility;

#[cfg(test)]
mod tests;

pub use action::DialogueAction;
pub use config::{DialogueActionKind, DialogueOptionRule, UnitDialogueConfig};
pub use content::DialogueContent;
pub use eligibility::{
    DialogueOptionAvailability, DialogueUnavailableReason, DIALOGUE_INTERACTION_RANGE_METERS,
    evaluate_dialogue_option, present_dialogue_options, unit_supports_dialogue,
    units_within_dialogue_range,
};
