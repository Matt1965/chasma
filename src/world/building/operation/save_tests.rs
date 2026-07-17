//! Operation save-state round trip (ADR-106 TF6).

use crate::world::building::operation::{
    BuildingOperationSaveState, BuildingOperationStore, ProductionProgress,
};

#[test]
fn operation_progress_round_trips_through_save_state() {
    let mut store = BuildingOperationStore::default();
    let building_id = crate::world::BuildingId::new(42);
    {
        let state = store.get_or_default_mut(building_id);
        state.progress = ProductionProgress(123_456);
        state.completion_count = 2;
        state.last_efficiency_revision = 99;
    }
    let exported = store.export_save_state();
    let mut restored = BuildingOperationStore::default();
    restored.import_save_state(exported);
    let state = restored.get(building_id).unwrap();
    assert_eq!(state.progress.value(), 123_456);
    assert_eq!(state.completion_count, 2);
    assert_eq!(state.last_efficiency_revision, 99);
}
