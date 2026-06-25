//! Command palette — available commands per selection (ADR-041 U-UI5).

use crate::units::input::SelectedUnits;
use crate::world::{UnitCatalog, UnitId};

use super::command_types::CommandType;

/// One entry in the command palette (UI / hotkey hook).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandPaletteEntry {
    pub command_type: CommandType,
    pub enabled: bool,
}

/// Static U-UI5 palette: Move, Stop, Hold Position.
///
/// Mixed-capability selections fall back to the intersection (currently identical for all units).
pub fn available_commands_for_selection(
    selection: &SelectedUnits,
    _catalog: &UnitCatalog,
) -> Vec<CommandPaletteEntry> {
    if selection.is_empty() {
        return Vec::new();
    }

    static PALETTE: [CommandType; 3] =
        [CommandType::Move, CommandType::Stop, CommandType::HoldPosition];

    PALETTE
        .iter()
        .map(|&command_type| CommandPaletteEntry {
            command_type,
            enabled: command_enabled_for_selection(command_type, selection),
        })
        .collect()
}

fn command_enabled_for_selection(command_type: CommandType, _selection: &SelectedUnits) -> bool {
    match command_type {
        CommandType::Move | CommandType::Stop => true,
        CommandType::HoldPosition => true,
        CommandType::AttackMove | CommandType::Interact | CommandType::Attack => false,
    }
}

/// Per-unit capability hook for future ability expansion.
pub fn unit_supports_command(_unit_id: UnitId, command_type: CommandType) -> bool {
    match command_type {
        CommandType::Move | CommandType::Stop | CommandType::HoldPosition => true,
        CommandType::AttackMove | CommandType::Interact | CommandType::Attack => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_selection_has_no_palette_entries() {
        let selection = SelectedUnits::default();
        assert!(available_commands_for_selection(&selection, &UnitCatalog::default()).is_empty());
    }

    #[test]
    fn selection_exposes_static_palette() {
        let mut selection = SelectedUnits::default();
        selection.set_single(crate::world::UnitId::new(1));
        let entries = available_commands_for_selection(&selection, &UnitCatalog::default());
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].command_type, CommandType::Move);
        assert_eq!(entries[1].command_type, CommandType::Stop);
        assert_eq!(entries[2].command_type, CommandType::HoldPosition);
    }

    #[test]
    fn multi_unit_mixed_capabilities_still_expose_move() {
        let mut selection = SelectedUnits::default();
        selection.replace_with([crate::world::UnitId::new(1), crate::world::UnitId::new(2)]);
        let entries = available_commands_for_selection(&selection, &UnitCatalog::default());
        assert!(entries.iter().any(|e| e.command_type == CommandType::Move && e.enabled));
    }
}
