//! Centralized session transition requests (client-local).

use bevy::prelude::*;

use super::navigation::MenuNavigation;
use super::screen::{AppScreen, GameSessionKind, GameSessionState};
use crate::simulation::SimulationControlState;
use crate::world::{
    AppearanceProfileCatalog, InventoryCatalogCtx, ItemCatalog, ItemCategoryCatalog,
    InventoryProfileCatalog, OriginCatalog, UnitCatalog, WorldData,
};

use super::starting_squad::{
    PendingStartingSquadSpawn, pending_camera_focus_for_anchor, spawn_starting_squad_from_draft,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTransitionKind {
    StartNewGame,
    StartDefaultWorldAuthoring,
    ReturnToMainMenu,
}

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

pub fn apply_session_transition_requests(
    mut request: ResMut<SessionTransitionRequest>,
    mut next_screen: ResMut<NextState<AppScreen>>,
    mut session: ResMut<GameSessionState>,
    mut nav: ResMut<MenuNavigation>,
    mut control: ResMut<SimulationControlState>,
    mut loading: ResMut<super::loading::LoadingSession>,
    mut world: ResMut<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    item_catalog: Res<ItemCatalog>,
    item_categories: Res<ItemCategoryCatalog>,
    inventory_profiles: Res<InventoryProfileCatalog>,
    origins: Res<OriginCatalog>,
    pending_spawn: Option<Res<PendingStartingSquadSpawn>>,
    mut commands: Commands,
) {
    let Some(kind) = request.take() else {
        return;
    };
    match kind {
        SessionTransitionKind::StartNewGame => {
            session.kind = GameSessionKind::NewGame;
            if let Some(pending) = pending_spawn {
                if let Some(origin) = origins.get(&pending.draft.origin_id) {
                    let inventory_ctx = InventoryCatalogCtx::new(
                        &item_catalog,
                        &item_categories,
                        &inventory_profiles,
                    );
                    let anchor = origin.spawn_anchor();
                    if let Ok(spawned) = spawn_starting_squad_from_draft(
                        &mut world,
                        &unit_catalog,
                        &appearance_profiles,
                        &inventory_ctx,
                        &anchor,
                        &pending.draft,
                    ) {
                        if !spawned.is_empty() {
                            commands.insert_resource(
                                pending_camera_focus_for_anchor(&anchor, world.layout()),
                            );
                        }
                    }
                }
                commands.remove_resource::<PendingStartingSquadSpawn>();
            }
            loading.begin(GameSessionKind::NewGame);
            control.pause();
            nav.close_pause();
            next_screen.set(AppScreen::Loading);
        }
        SessionTransitionKind::StartDefaultWorldAuthoring => {
            session.kind = GameSessionKind::DefaultWorldAuthoring;
            loading.begin(GameSessionKind::DefaultWorldAuthoring);
            control.pause();
            nav.close_pause();
            next_screen.set(AppScreen::Loading);
        }
        SessionTransitionKind::ReturnToMainMenu => {
            control.pause();
            session.clear();
            commands.remove_resource::<PendingStartingSquadSpawn>();
            nav.open_main_root();
            next_screen.set(AppScreen::MainMenu);
        }
    }
}
