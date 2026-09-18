//! Origin selection input handlers (CG7).

use bevy::prelude::*;

use crate::menu::{
    AppScreen, GameSessionState, SessionTransitionKind, SessionTransitionRequest,
};
use crate::world::OriginCatalog;

use super::screen::{OriginSelectAction, OriginSelectListButton};
use super::session::OriginSelectSession;

pub fn handle_origin_select_buttons(
    mut interaction: Query<
        (
            &Interaction,
            Option<&OriginSelectAction>,
            Option<&OriginSelectListButton>,
            &mut BackgroundColor,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut session: ResMut<OriginSelectSession>,
    mut game_session: ResMut<GameSessionState>,
    origins: Res<OriginCatalog>,
    mut transitions: ResMut<SessionTransitionRequest>,
    mut next_screen: ResMut<NextState<AppScreen>>,
) {
    for (interaction, action, list_button, mut bg) in &mut interaction {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(Color::srgb(0.28, 0.36, 0.46));
                if let Some(OriginSelectListButton { index }) = list_button {
                    session.selected_index = *index;
                }
                if let Some(action) = action {
                    match action {
                        OriginSelectAction::Back => {
                            next_screen.set(AppScreen::MainMenu);
                        }
                        OriginSelectAction::Continue => {
                            game_session.selected_origin_id =
                                session.selected_origin_id(&origins);
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

pub fn cleanup_origin_select_session(mut commands: Commands) {
    commands.remove_resource::<OriginSelectSession>();
}
