//! Unit Editor open/close, Done/Cancel, and Escape handling (CG3).

use bevy::prelude::*;

use crate::menu::AppScreen;
use crate::world::{AppearanceProfileCatalog, UnitCatalog, UnitId, WorldData};

use super::commit::commit_live_unit_appearance;
use super::draft::UnitAppearanceDraft;
use super::screen::{UnitEditorAction, UnitEditorActionButton};
use super::session::{UnitEditorMode, UnitEditorSession};

/// Open the editor for a live unit (Dev entry and future CG8 reuse).
pub fn open_unit_editor_for_live_unit(
    commands: &mut Commands,
    next_screen: &mut ResMut<NextState<AppScreen>>,
    unit_id: UnitId,
    world: &WorldData,
    catalog: &UnitCatalog,
    profiles: &AppearanceProfileCatalog,
) -> Result<(), String> {
    let record = world
        .get_unit(unit_id)
        .ok_or_else(|| format!("unit {} no longer exists", unit_id.raw()))?;
    let definition = catalog
        .get(&record.definition_id)
        .ok_or_else(|| format!("missing definition `{}`", record.definition_id.as_str()))?;
    let appearance = record
        .appearance
        .clone()
        .ok_or_else(|| "unit has no appearance data".to_string())?;
    if definition.appearance_profile_id.is_none() {
        return Err("unit definition does not support appearance editing".into());
    }
    profiles
        .get(&appearance.profile_id)
        .ok_or_else(|| format!("unknown appearance profile `{}`", appearance.profile_id.as_str()))?;

    let draft = UnitAppearanceDraft::from_live(
        record.definition_id.clone(),
        Some(definition.display_name.clone()),
        appearance,
    );
    commands.insert_resource(UnitEditorSession::new(UnitEditorMode::LiveUnit(unit_id), draft));
    next_screen.set(AppScreen::UnitEditor);
    Ok(())
}

pub fn handle_unit_editor_buttons(
    session: Option<ResMut<UnitEditorSession>>,
    mut next_screen: ResMut<NextState<AppScreen>>,
    mut world: ResMut<WorldData>,
    catalog: Res<UnitCatalog>,
    profiles: Res<AppearanceProfileCatalog>,
    buttons: Query<(&Interaction, &UnitEditorActionButton), Changed<Interaction>>,
) {
    let Some(mut session) = session else {
        return;
    };
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button.action {
            UnitEditorAction::Cancel => {
                commands_remove_session(&mut next_screen);
                return;
            }
            UnitEditorAction::Done => {
                let unit_id = session.live_unit_id();
                let draft = session.draft.clone();
                let result = unit_id.map(|id| {
                    commit_live_unit_appearance(&mut world, &catalog, &profiles, id, &draft)
                        .map_err(|error| error.to_string())
                });
                match result {
                    Some(Ok(())) => {
                        commands_remove_session(&mut next_screen);
                        return;
                    }
                    Some(Err(message)) => session.error_message = Some(message),
                    None => session.error_message = Some("editor session is missing a live unit".into()),
                }
            }
        }
    }
}

pub fn handle_unit_editor_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    session: Option<Res<UnitEditorSession>>,
    mut next_screen: ResMut<NextState<AppScreen>>,
) {
    if session.is_none() || !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    commands_remove_session(&mut next_screen);
}

pub fn cleanup_unit_editor_session(mut commands: Commands, session: Option<Res<UnitEditorSession>>) {
    if session.is_some() {
        commands.remove_resource::<UnitEditorSession>();
    }
}

fn commands_remove_session(next_screen: &mut ResMut<NextState<AppScreen>>) {
    next_screen.set(AppScreen::InGame);
}
