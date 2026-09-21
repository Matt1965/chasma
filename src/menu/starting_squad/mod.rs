//! Starting-squad draft and spawn seam for New Game (CG8).

use bevy::prelude::Resource;

mod camera;
mod draft;
mod session;
mod spawn;

#[cfg(test)]
mod tests;

pub use draft::{
    SquadMemberDraft, SquadMemberDraftId, StartingSquadDraft, build_starting_squad_draft,
};
pub use camera::{
    PendingNewGameCameraFocus, focus_camera_on_new_game_spawn, pending_camera_focus_for_anchor,
};
pub use session::{OriginSquadViewMode, StartingSquadSession};
pub use spawn::spawn_starting_squad_from_draft;

/// Draft queued for spawn when Begin Game transitions into loading.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct PendingStartingSquadSpawn {
    pub draft: StartingSquadDraft,
}
