//! TF3 overlay state and color mapping tests.

use bevy::prelude::*;

use crate::terrain::field_overlay::state::{MAX_PLAYER_OVERLAY_OPACITY_BP, TerrainOverlayState};
use crate::world::{TerrainFieldId, TerrainFieldOverlayStyle};

#[test]
fn overlay_state_selecting_field_increments_revision() {
    let mut state = TerrainOverlayState::default();
    let before = state.request_revision;
    state.set_manual_field(Some(TerrainFieldId::new("water")));
    assert_eq!(state.request_revision, before + 1);
    assert_eq!(state.effective_field().map(|id| id.as_str()), Some("water"));
}

#[test]
fn opacity_clamps_to_max() {
    let mut state = TerrainOverlayState::default();
    state.set_opacity_basis_points(20_000);
    assert_eq!(state.opacity_basis_points, MAX_PLAYER_OVERLAY_OPACITY_BP);
}

#[test]
fn unknown_color_differs_from_zero_value() {
    let style = TerrainFieldOverlayStyle {
        visibility_cutoff: 100,
        ..Default::default()
    };
    let zero = style.vertex_color_for_value(0, 5_500);
    let unknown = style.unknown_vertex_color(5_500, true);
    assert!(zero.alpha() < 0.01);
    assert!(unknown.alpha() > 0.1);
    assert_ne!(zero, unknown);
}

#[test]
fn qualitative_labels_use_threshold_bands() {
    let style = TerrainFieldOverlayStyle {
        qualitative_thresholds: vec![100, 200],
        qualitative_labels: vec!["Low".into(), "Mid".into(), "High".into()],
        ..Default::default()
    };
    assert_eq!(style.qualitative_label_for_value(50), Some("Low"));
    assert_eq!(style.qualitative_label_for_value(150), Some("Mid"));
    assert_eq!(style.qualitative_label_for_value(250), Some("High"));
}
