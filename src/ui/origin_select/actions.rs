//! Continuous origin/squad stage input handlers (CG8).

use bevy::prelude::*;

use crate::menu::{
    AppScreen, GameSessionState, PendingStartingSquadSpawn, SessionTransitionKind,
    SessionTransitionRequest,     StartingSquadSession,
};
use crate::world::{AppearanceProfileCatalog, OriginCatalog, UnitCatalog};

use super::focus::open_focus_editor_for_slot;
use super::screen::{OriginSquadAction, OriginSquadMemberButton};

pub fn init_starting_squad_session_on_enter(
    mut commands: Commands,
    origins: Res<OriginCatalog>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
) {
    match StartingSquadSession::new_for_first_origin(
        &origins,
        &unit_catalog,
        &appearance_profiles,
    ) {
        Ok(session) => commands.insert_resource(session),
        Err(error) => warn!("failed to init starting squad session: {error}"),
    }
}

pub fn handle_origin_squad_buttons(
    mut interaction: Query<
        (
            &Interaction,
            Option<&OriginSquadAction>,
            Option<&OriginSquadMemberButton>,
            &mut BackgroundColor,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut session: ResMut<StartingSquadSession>,
    mut game_session: ResMut<GameSessionState>,
    origins: Res<OriginCatalog>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    mut transitions: ResMut<SessionTransitionRequest>,
    mut next_screen: ResMut<NextState<AppScreen>>,
    mut commands: Commands,
) {
    if session.is_focused() {
        return;
    }
    for (interaction, action, member_button, mut bg) in &mut interaction {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(Color::srgb(0.28, 0.36, 0.46));
                if let Some(OriginSquadMemberButton { slot_index }) = member_button {
                    if let Err(error) = session.enter_focus(*slot_index) {
                        warn!("focus failed: {error}");
                        continue;
                    }
                    if let Err(error) = open_focus_editor_for_slot(
                        &mut commands,
                        &session,
                        *slot_index,
                        &origins,
                        &unit_catalog,
                        &appearance_profiles,
                    ) {
                        session.exit_focus();
                        warn!("focus editor open failed: {error}");
                    }
                }
                if let Some(action) = action {
                    match action {
                        OriginSquadAction::Back => {
                            commands.remove_resource::<StartingSquadSession>();
                            next_screen.set(AppScreen::MainMenu);
                        }
                        OriginSquadAction::PrevOrigin => {
                            if let Err(error) = session.cycle_origin(
                                -1,
                                &origins,
                                &unit_catalog,
                                &appearance_profiles,
                            ) {
                                warn!("origin cycle failed: {error}");
                            }
                        }
                        OriginSquadAction::NextOrigin => {
                            if let Err(error) = session.cycle_origin(
                                1,
                                &origins,
                                &unit_catalog,
                                &appearance_profiles,
                            ) {
                                warn!("origin cycle failed: {error}");
                            }
                        }
                        OriginSquadAction::BeginGame => {
                            let Some(draft) = session.active_draft(&origins).cloned() else {
                                warn!("begin game blocked: no active draft");
                                continue;
                            };
                            game_session.selected_origin_id =
                                session.active_origin_id(&origins);
                            commands.insert_resource(PendingStartingSquadSpawn { draft });
                            transitions.request(SessionTransitionKind::StartNewGame);
                        }
                    }
                }
            }
            Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.22, 0.28, 0.36)),
            Interaction::None => *bg = BackgroundColor(Color::srgb(0.16, 0.2, 0.26)),
        }
    }
}

pub fn respawn_origin_squad_ui_after_focus(
    commands: Commands,
    session: Res<StartingSquadSession>,
    squad_panels: Query<Entity, With<super::screen::OriginSelectSquadPanelRoot>>,
    focus_roots: Query<Entity, With<super::focus::OriginSquadFocusUiRoot>>,
    origins: Res<OriginCatalog>,
) {
    if session.is_focused() || !focus_roots.is_empty() || !squad_panels.is_empty() {
        return;
    }
    super::screen::spawn_origin_select_squad_panel(commands, origins, session);
}

pub fn cleanup_origin_select_session(mut commands: Commands) {
    commands.remove_resource::<StartingSquadSession>();
    commands.remove_resource::<crate::ui::unit_editor::UnitEditorSession>();
}
