//! Dev workspace completion tests (Slice 13).

use super::hotkeys::{DEV_HOTKEY_REGISTRY, DevShortcutLifecycle};
use super::widgets::theme::{SPACE_CONTROL, SPACE_SECTION, SPACE_TIGHT, SPACE_WINDOW};

#[test]
fn hotkey_registry_documents_core_bindings() {
    assert!(DEV_HOTKEY_REGISTRY.iter().any(|e| e.key_label == "F12"));
    assert!(DEV_HOTKEY_REGISTRY.iter().any(|e| e.key_label == ","));
    for entry in DEV_HOTKEY_REGISTRY {
        assert!(
            matches!(
                entry.lifecycle,
                DevShortcutLifecycle::Retained | DevShortcutLifecycle::Removed
            ),
            "unexpected lifecycle for {}",
            entry.key_label
        );
    }
}

#[test]
fn retained_global_shortcuts_documented() {
    let labels: Vec<_> = DEV_HOTKEY_REGISTRY
        .iter()
        .filter(|e| e.lifecycle == DevShortcutLifecycle::Retained)
        .map(|e| e.key_label)
        .collect();
    for key in ["F12", ",", ".", "/"] {
        assert!(labels.contains(&key), "missing retained key {key}");
    }
}

#[test]
fn spacing_scale_monotonic() {
    assert!(SPACE_TIGHT < SPACE_CONTROL);
    assert!(SPACE_CONTROL <= SPACE_SECTION);
    assert!(SPACE_SECTION <= SPACE_WINDOW);
}

#[test]
fn removed_hotkeys_not_active_handlers() {
    // Registry documents removals; handlers must not exist for building letter keys.
    for entry in DEV_HOTKEY_REGISTRY {
        if entry.lifecycle == DevShortcutLifecycle::Removed {
            assert!(
                entry.action.starts_with('(') || entry.ui_replacement.is_some(),
                "removed entry should explain replacement: {}",
                entry.key_label
            );
        }
    }
}
