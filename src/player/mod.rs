//! Client-local player control and presentation.

mod box_select_overlay;
mod indicator;
mod move_feedback;
mod ownership;
mod plugin;
mod selection_policy;
mod selection_ring_mesh;
mod simulation;
mod space_view;

pub use move_feedback::MoveCommandFeedback;
pub use ownership::{LocalPlayerOwnership, selection_policy_for_frame};
pub use plugin::{
    DebugPresentationSystems, GameplayPresentationSystems, PlayerControlSystems, PlayerPlugin,
    RuntimeSyncSystems,
};
pub use selection_policy::SelectionPolicyState;
pub use selection_ring_mesh::selection_ring_radius;
pub use space_view::{ActiveViewedSpace, ViewFollowLock, sync_active_viewed_space};
