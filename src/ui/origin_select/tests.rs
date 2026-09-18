use super::session::OriginSelectSession;
use crate::units::presentation::roster_preview_offsets;
use crate::world::{OriginCatalog, starter_origin_catalog};

#[test]
fn roster_preview_offsets_center_pair() {
    let offsets = roster_preview_offsets(2, 1.5);
    assert_eq!(offsets.len(), 2);
    assert!(offsets[0].x < 0.0);
    assert!(offsets[1].x > 0.0);
}

#[test]
fn origin_session_resolves_selected_id() {
    let catalog = starter_origin_catalog();
    let session = OriginSelectSession {
        selected_index: 1,
        preview_yaw_radians: 0.0,
        preview_zoom: 1.0,
    };
    assert_eq!(
        session
            .selected_origin_id(&catalog)
            .map(|id| id.as_str().to_string()),
        Some("lone_survivor".to_string())
    );
}

#[test]
fn starter_catalog_is_valid_resource() {
    let _: OriginCatalog = starter_origin_catalog();
}
