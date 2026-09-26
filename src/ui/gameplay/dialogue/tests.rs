use super::state::{DialogueSessionState, DialogueView};
use crate::world::{DialogueAction, DialogueActionKind, DialogueContent};

#[test]
fn selecting_talk_produces_hi_and_back_returns_to_options() {
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
    session.back_to_options();
    assert!(matches!(session.view, DialogueView::Options));
}

#[test]
fn trade_and_recruit_emit_placeholder_actions() {
    let mut session = DialogueSessionState::default();
    session.open_session(crate::world::UnitId::new(1), crate::world::UnitId::new(2));
    session.select_option(
        DialogueActionKind::Trade,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
    );
    assert!(matches!(session.view, DialogueView::TradePlaceholder));
    assert!(matches!(
        session.last_action,
        Some(DialogueAction::Trade { .. })
    ));
    session.back_to_options();
    session.select_option(
        DialogueActionKind::Recruit,
        crate::world::UnitId::new(1),
        crate::world::UnitId::new(2),
    );
    assert!(matches!(session.view, DialogueView::RecruitPlaceholder));
}
