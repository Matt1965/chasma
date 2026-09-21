use bevy::prelude::*;

#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct DevOriginEditorState {
    pub selected_index: usize,
    pub dirty: bool,
    pub pending_delete: bool,
    pub scratch_name: String,
    pub scratch_description: String,
    pub pending_start_pick: bool,
    pub status_message: String,
}

impl DevOriginEditorState {
    pub fn sync_scratch_from_definition(
        &mut self,
        display_name: &str,
        description: &str,
    ) {
        self.scratch_name = display_name.to_string();
        self.scratch_description = description.to_string();
    }
}
