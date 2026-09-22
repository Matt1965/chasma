//! Focused member editing on the continuous origin/squad stage (CG8).

use bevy::prelude::*;

use crate::menu::StartingSquadSession;
use crate::ui::unit_editor::{
    UnitEditorAction, UnitEditorActionButton, UnitEditorMode, UnitEditorSession,
    spawn_unit_editor_controls_panel,
};
use crate::world::{AppearanceProfileCatalog, OriginCatalog, UnitCatalog};

#[derive(Component, Debug)]
pub struct OriginSquadFocusUiRoot;

pub fn open_focus_editor_for_slot(
    commands: &mut Commands,
    squad_session: &StartingSquadSession,
    slot_index: usize,
    origins: &OriginCatalog,
    unit_catalog: &UnitCatalog,
    profiles: &AppearanceProfileCatalog,
) -> Result<(), String> {
    let draft = squad_session
        .active_draft(origins)
        .ok_or_else(|| "missing active squad draft".to_string())?;
    let member = draft
        .member_by_slot(slot_index)
        .ok_or_else(|| format!("missing squad member slot {slot_index}"))?;
    let definition = unit_catalog
        .get(&member.definition_id)
        .ok_or_else(|| format!("missing definition `{}`", member.definition_id.as_str()))?;
    if definition.appearance_profile_id.is_none() {
        return Err("unit definition does not support appearance editing".into());
    }
    profiles
        .get(&member.appearance.appearance.profile_id)
        .ok_or_else(|| {
            format!(
                "unknown appearance profile `{}`",
                member.appearance.appearance.profile_id.as_str()
            )
        })?;
    let mut editor = UnitEditorSession::new(
        UnitEditorMode::NewGameDraft { slot_index },
        member.appearance.clone(),
    );
    editor.preview_yaw_radians = squad_session.preview_yaw_radians;
    editor.preview_zoom = squad_session.preview_zoom;
    commands.insert_resource(editor);
    Ok(())
}

pub fn sync_origin_squad_focus_ui(
    mut commands: Commands,
    session: Res<StartingSquadSession>,
    editor_session: Option<Res<UnitEditorSession>>,
    squad_panels: Query<Entity, With<super::screen::OriginSelectSquadPanelRoot>>,
    focus_roots: Query<Entity, With<OriginSquadFocusUiRoot>>,
    profiles: Res<AppearanceProfileCatalog>,
) {
    let want_focus = session.is_focused();
    let has_focus_ui = !focus_roots.is_empty();

    if want_focus && !has_focus_ui {
        for entity in &squad_panels {
            commands.entity(entity).despawn();
        }
        if let Some(editor) = editor_session.as_ref() {
            commands
                .spawn((
                    OriginSquadFocusUiRoot,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    ZIndex(210),
                ))
                .with_children(|root| {
                    spawn_unit_editor_controls_panel(root, editor, &profiles);
                });
        }
    } else if !want_focus && has_focus_ui {
        for entity in &focus_roots {
            commands.entity(entity).despawn();
        }
    }
}

pub fn handle_origin_squad_focus_done(
    mut squad_session: ResMut<StartingSquadSession>,
    editor_session: Option<ResMut<UnitEditorSession>>,
    origins: Res<OriginCatalog>,
    buttons: Query<(&Interaction, &UnitEditorActionButton), Changed<Interaction>>,
    mut commands: Commands,
) {
    if !squad_session.is_focused() {
        return;
    }
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed || button.action != UnitEditorAction::Done {
            continue;
        }
        let Some(editor) = editor_session else {
            squad_session.exit_focus();
            return;
        };
        let slot_index = match editor.mode {
            UnitEditorMode::NewGameDraft { slot_index } => slot_index,
            _ => {
                squad_session.exit_focus();
                return;
            }
        };
        let draft_snapshot = editor.draft.clone();
        let initial = editor.initial_draft.clone();
        if let Some(draft) = squad_session.active_draft_mut(&origins) {
            if let Some(member) = draft.member_by_slot_mut(slot_index) {
                member.appearance = draft_snapshot;
                member.edited = member.appearance != initial;
            }
        }
        squad_session.preview_yaw_radians = editor.preview_yaw_radians;
        squad_session.preview_zoom = editor.preview_zoom;
        squad_session.exit_focus();
        commands.remove_resource::<UnitEditorSession>();
        return;
    }
}

pub fn despawn_origin_squad_focus_ui(
    mut commands: Commands,
    roots: Query<Entity, With<OriginSquadFocusUiRoot>>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
