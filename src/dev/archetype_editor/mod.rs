//! Dev editor authoring for unit/building archetype templates.

mod actions;
mod capture_preview;
mod keyboard;
mod modal;
mod state;

pub use actions::{
    DevArchetypeEditorScratch, DevArchetypeEditButton, DevArchetypeSaveButton,
    handle_archetype_edit_button, handle_archetype_modal_cancel, handle_archetype_modal_delete,
    handle_archetype_modal_save, handle_archetype_save_button, handle_archetype_species_toggle,
};
pub use keyboard::handle_archetype_editor_keyboard;
pub use capture_preview::{
    draw_building_archetype_capture_preview, sync_building_archetype_capture_preview,
};
pub use modal::{
    handle_archetype_modal_field_clicks, setup_archetype_editor_modal,
    sync_archetype_editor_modal, sync_archetype_modal_field_styles,
    sync_archetype_species_toggle_marks,
};
pub use state::DevArchetypeEditorState;
