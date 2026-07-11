//! Command availability and structured unavailability reasons (REVIEW-B3, ADR-039).

use crate::units::input::SelectedUnits;

use super::command_types::CommandType;

/// Why a command cannot be issued right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandUnavailableReason {
    EmptySelection,
    FeatureNotImplemented,
    RequiresTerrainTarget,
    RequiresUnitTarget,
}

impl CommandUnavailableReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::EmptySelection => "No units selected",
            Self::FeatureNotImplemented => "Not implemented",
            Self::RequiresTerrainTarget => "Requires terrain target",
            Self::RequiresUnitTarget => "Requires unit target",
        }
    }
}

/// Whether a command may be issued for the current selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAvailability {
    Available,
    Unavailable(CommandUnavailableReason),
}

impl CommandAvailability {
    pub fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }

    pub fn reason(self) -> Option<CommandUnavailableReason> {
        match self {
            Self::Available => None,
            Self::Unavailable(reason) => Some(reason),
        }
    }
}

/// Central availability rules for the player command set (REVIEW-B3).
pub fn command_availability(
    command_type: CommandType,
    selection: &SelectedUnits,
) -> CommandAvailability {
    if selection.is_empty() {
        return CommandAvailability::Unavailable(CommandUnavailableReason::EmptySelection);
    }

    match command_type {
        CommandType::Move | CommandType::Stop | CommandType::Attack | CommandType::AttackMove => {
            CommandAvailability::Available
        }
        CommandType::HoldPosition | CommandType::Interact => {
            CommandAvailability::Unavailable(CommandUnavailableReason::FeatureNotImplemented)
        }
    }
}

/// Tooltip text for HUD / palette presentation.
pub fn command_tooltip(command_type: CommandType, availability: CommandAvailability) -> String {
    let base = command_type.description();
    if let Some(reason) = availability.reason() {
        format!("{base} ({})", reason.label())
    } else {
        base.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implemented_commands_available_with_selection() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        assert!(command_availability(CommandType::Move, &selection).is_available());
        assert!(command_availability(CommandType::Stop, &selection).is_available());
        assert!(command_availability(CommandType::Attack, &selection).is_available());
        assert!(command_availability(CommandType::AttackMove, &selection).is_available());
    }

    #[test]
    fn hold_and_interact_report_feature_not_implemented() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let hold = command_availability(CommandType::HoldPosition, &selection);
        let interact = command_availability(CommandType::Interact, &selection);
        assert_eq!(
            hold,
            CommandAvailability::Unavailable(CommandUnavailableReason::FeatureNotImplemented)
        );
        assert_eq!(
            interact,
            CommandAvailability::Unavailable(CommandUnavailableReason::FeatureNotImplemented)
        );
    }

    #[test]
    fn empty_selection_makes_all_commands_unavailable() {
        let selection = SelectedUnits::default();
        assert_eq!(
            command_availability(CommandType::Move, &selection),
            CommandAvailability::Unavailable(CommandUnavailableReason::EmptySelection)
        );
    }

    #[test]
    fn tooltip_includes_unavailable_reason() {
        let tooltip = command_tooltip(
            CommandType::HoldPosition,
            CommandAvailability::Unavailable(CommandUnavailableReason::FeatureNotImplemented),
        );
        assert!(tooltip.contains("Not implemented"));
    }
}
