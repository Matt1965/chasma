//! Centralized session transition requests (client-local).
//!
//! Button handlers only set this resource. Apply systems perform screen/session
//! changes without clearing WorldData or reloading terrain.

use bevy::prelude::*;

use super::navigation::MenuNavigation;
use super::screen::{AppScreen, GameSessionKind, GameSessionState};
use crate::simulation::SimulationControlState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTransitionKind {
    StartNewGame,
    StartDefaultWorldAuthoring,
    ReturnToMainMenu,
}

/// Queued session transition. Consumed once per apply.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SessionTransitionRequest {
    pub pending: Option<SessionTransitionKind>,
}

impl SessionTransitionRequest {
    pub fn request(&mut self, kind: SessionTransitionKind) {
        self.pending = Some(kind);
    }

    pub fn take(&mut self) -> Option<SessionTransitionKind> {
        self.pending.take()
    }
}

/// Apply pending session transitions. Does not clear or reload WorldData.
pub fn apply_session_transition_requests(
    mut request: ResMut<SessionTransitionRequest>,
    mut next_screen: ResMut<NextState<AppScreen>>,
    mut session: ResMut<GameSessionState>,
    mut nav: ResMut<MenuNavigation>,
    mut control: ResMut<SimulationControlState>,
    mut loading: ResMut<super::loading::LoadingSession>,
) {
    let Some(kind) = request.take() else {
        return;
    };
    match kind {
        SessionTransitionKind::StartNewGame => {
            // Temporary bridge: reuse the initialized runtime world.
            session.kind = GameSessionKind::NewGame;
            loading.begin(GameSessionKind::NewGame);
            control.pause();
            nav.close_pause();
            next_screen.set(AppScreen::Loading);
        }
        SessionTransitionKind::StartDefaultWorldAuthoring => {
            // Temporary bridge: reuse the initialized runtime world as authoring session.
            session.kind = GameSessionKind::DefaultWorldAuthoring;
            loading.begin(GameSessionKind::DefaultWorldAuthoring);
            control.pause();
            nav.close_pause();
            next_screen.set(AppScreen::Loading);
        }
        SessionTransitionKind::ReturnToMainMenu => {
            // Preserve WorldData in memory; pause and hide gameplay via screen change.
            control.pause();
            session.clear();
            nav.open_main_root();
            next_screen.set(AppScreen::MainMenu);
        }
    }
}
