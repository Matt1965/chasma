use super::state::{DialogueSessionState, DialogueView};
use crate::world::{DialogueAction, DialogueActionKind, DialogueContent};

#[test]
fn selecting_talk_shows_greeting_without_options_back_step() {
    let mut session = DialogueSessionState::default();
    session.open_session(crate::world::UnitId::new(1), crate::world::UnitId::new(2));
    session.select_option(
        DialogueActionKind::Talk,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
    );
    assert!(matches!(
        session.view,
        DialogueView::TalkResponse(ref content) if *content == DialogueContent::default_greeting()
    ));
    assert!(matches!(
        session.last_action,
        Some(DialogueAction::Talk { .. })
    ));
    assert!(!matches!(session.view, DialogueView::Options));
}

#[test]
fn recruit_still_uses_placeholder_view() {
    let mut session = DialogueSessionState::default();
    session.open_session(crate::world::UnitId::new(1), crate::world::UnitId::new(2));
    session.select_option(
        DialogueActionKind::Recruit,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
    );
    assert!(matches!(session.view, DialogueView::RecruitPlaceholder));
    session.back_to_options();
    assert!(matches!(session.view, DialogueView::Options));
}
