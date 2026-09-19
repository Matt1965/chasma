use bevy::prelude::*;

use crate::dev::dev_mode::{DevModeState, DevTextFieldFocus};

use super::state::DevArchetypeEditorState;

pub fn handle_archetype_editor_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut dev_state: ResMut<DevModeState>,
    mut editor: ResMut<DevArchetypeEditorState>,
) {
    if !editor.modal_open || !dev_state.enabled {
        return;
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        dev_state.clear_text_focus();
        return;
    }

    let target = match dev_state.text_focus {
        DevTextFieldFocus::ArchetypeName => Some(&mut editor.name_input),
        DevTextFieldFocus::ArchetypeGoldMin => Some(&mut editor.gold_min_input),
        DevTextFieldFocus::ArchetypeGoldMax => Some(&mut editor.gold_max_input),
        _ => None,
    };
    let Some(buffer) = target else {
        return;
    };

    if keyboard.just_pressed(KeyCode::Backspace) {
        buffer.pop();
        return;
    }

    for key in keyboard.get_just_pressed() {
        if let Some(ch) = key_to_char(*key) {
            if dev_state.text_focus == DevTextFieldFocus::ArchetypeName || buffer.len() < 12 {
                buffer.push(ch);
            }
        }
    }
}

fn key_to_char(key: KeyCode) -> Option<char> {
    match key {
        KeyCode::Space => Some(' '),
        KeyCode::Minus => Some('-'),
        KeyCode::Period => Some('.'),
        KeyCode::KeyA => Some('a'),
        KeyCode::KeyB => Some('b'),
        KeyCode::KeyC => Some('c'),
        KeyCode::KeyD => Some('d'),
        KeyCode::KeyE => Some('e'),
        KeyCode::KeyF => Some('f'),
        KeyCode::KeyG => Some('g'),
        KeyCode::KeyH => Some('h'),
        KeyCode::KeyI => Some('i'),
        KeyCode::KeyJ => Some('j'),
        KeyCode::KeyK => Some('k'),
        KeyCode::KeyL => Some('l'),
        KeyCode::KeyM => Some('m'),
        KeyCode::KeyN => Some('n'),
        KeyCode::KeyO => Some('o'),
        KeyCode::KeyP => Some('p'),
        KeyCode::KeyQ => Some('q'),
        KeyCode::KeyR => Some('r'),
        KeyCode::KeyS => Some('s'),
        KeyCode::KeyT => Some('t'),
        KeyCode::KeyU => Some('u'),
        KeyCode::KeyV => Some('v'),
        KeyCode::KeyW => Some('w'),
        KeyCode::KeyX => Some('x'),
        KeyCode::KeyY => Some('y'),
        KeyCode::KeyZ => Some('z'),
        KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
        KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
        KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
        KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
        KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
        KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
        KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
        KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
        KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
        KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
        _ => None,
    }
}
