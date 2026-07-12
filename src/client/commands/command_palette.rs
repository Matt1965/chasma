//! Command palette — available commands per selection (ADR-041, REVIEW-B3).

use crate::units::input::SelectedUnits;
use crate::world::{UnitCatalog, UnitId};

use super::command_availability::command_availability;
use super::command_types::CommandType;

/// One entry in the command palette (UI / hotkey hook).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandPaletteEntry {
    pub command_type: CommandType,
    pub availability: super::command_availability::CommandAvailability,
}

impl CommandPaletteEntry {
    pub fn is_enabled(self) -> bool {
        self.availability.is_available()
    }
}

/// Player command palette for the current selection.
pub fn available_commands_for_selection(
    selection: &SelectedUnits,
    _catalog: &UnitCatalog,
) -> Vec<CommandPaletteEntry> {
    if selection.is_empty() {
        return Vec::new();
    }

    CommandType::player_palette()
        .iter()
        .map(|&command_type| CommandPaletteEntry {
            command_type,
            availability: command_availability(command_type, selection),
        })
        .collect()
}

/// Per-unit capability hook for future ability expansion.
pub fn unit_supports_command(_unit_id: UnitId, command_type: CommandType) -> bool {
    command_type.is_implemented()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::commands::CommandUnavailableReason;

    #[test]
    fn empty_selection_has_no_palette_entries() {
        let selection = SelectedUnits::default();
        assert!(available_commands_for_selection(&selection, &UnitCatalog::default()).is_empty());
    }

    #[test]
    fn selection_exposes_implemented_palette_commands() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let entries = available_commands_for_selection(&selection, &UnitCatalog::default());
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].command_type, CommandType::Move);
        assert!(entries[0].is_enabled());
        assert_eq!(entries[1].command_type, CommandType::Stop);
        assert!(!entries[2].is_enabled());
        assert_eq!(
            entries[2].availability.reason(),
            Some(CommandUnavailableReason::FeatureNotImplemented)
        );
        assert!(entries[3].is_enabled());
    }

    #[test]
    fn multi_unit_selection_still_exposes_move() {
        let mut selection = SelectedUnits::default();
        selection.replace_with([crate::world::UnitId::new(1), crate::world::UnitId::new(2)]);
        let entries = available_commands_for_selection(&selection, &UnitCatalog::default());
        assert!(
            entries
                .iter()
                .any(|e| e.command_type == CommandType::Move && e.is_enabled())
        );
    }

    #[test]
    fn hold_position_disabled_with_explicit_reason() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let hold = available_commands_for_selection(&selection, &UnitCatalog::default())
            .into_iter()
            .find(|e| e.command_type == CommandType::HoldPosition)
            .expect("hold entry");
        assert!(!hold.is_enabled());
        assert_eq!(
            hold.availability.reason(),
            Some(CommandUnavailableReason::FeatureNotImplemented)
        );
    }
}
