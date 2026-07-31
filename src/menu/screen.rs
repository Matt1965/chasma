//! Application screen state and session identity (client-local).

use bevy::prelude::*;

/// Top-level application screen. Registered once; plugins stay loaded.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppScreen {
    #[default]
    MainMenu,
    Loading,
    InGame,
}

/// How the current InGame/Loading session was requested.
///
/// `LoadedSave` is reserved for a future player-save provider and is unused here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameSessionKind {
    #[default]
    None,
    NewGame,
    LoadedSave,
    DefaultWorldAuthoring,
}

/// Client-local session identity. Not part of [`crate::world::WorldData`].
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameSessionState {
    pub kind: GameSessionKind,
}

impl GameSessionState {
    pub fn clear(&mut self) {
        self.kind = GameSessionKind::None;
    }
}
