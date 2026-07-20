//! Operation save-state round trip (ADR-106 TF6, EP2).

use crate::world::building::operation::{
    BuildingOperationPolicy, BuildingProductionStore, ControlSource, OperationDefinitionId,
    OperationLifecycle, ProductionProgress, RepeatMode,
};

#[test]
fn operation_progress_round_trips_through_save_state() {
    let mut store = BuildingProductionStore::default();
    let building_id = crate::world::BuildingId::new(42);
    store.set_policy(
        building_id,
        BuildingOperationPolicy {
            enabled: true,
            paused: false,
            selected_operation: Some(OperationDefinitionId::new("test_operation")),
            repeat_mode: RepeatMode::Continuous,
            priority: 64,
            control_source: ControlSource::PlayerControlled,
            planner_managed: false,
        },
    );
    {
        let state = store.get_or_default_mut(building_id);
        state.lifecycle = OperationLifecycle::Running;
        state.progress = ProductionProgress(123_456);
        state.completion_count = 2;
        state.last_efficiency_revision = 99;
        state.active_worker_count = 1;
    }
    let exported = store.export_save_state();
    let mut restored = BuildingProductionStore::default();
    restored.import_save_state(exported);
    let state = restored.get(building_id).unwrap();
    assert_eq!(state.lifecycle, OperationLifecycle::Running);
    assert_eq!(state.progress.value(), 123_456);
    assert_eq!(state.completion_count, 2);
    assert_eq!(state.last_efficiency_revision, 99);
    assert_eq!(state.active_worker_count, 1);
    let policy = restored.get_policy(building_id).unwrap();
    assert_eq!(
        policy.selected_operation.as_ref().map(|id| id.as_str()),
        Some("test_operation")
    );
    assert_eq!(policy.repeat_mode, RepeatMode::Continuous);
}

#[test]
fn save_state_preserves_operation_id_not_definition() {
    let mut store = BuildingProductionStore::default();
    let building_id = crate::world::BuildingId::new(7);
    let operation_id = OperationDefinitionId::new("mine_iron");
    store.set_policy(
        building_id,
        BuildingOperationPolicy {
            selected_operation: Some(operation_id.clone()),
            ..Default::default()
        },
    );
    let exported = store.export_save_state();
    let serialized = ron::ser::to_string(&exported).unwrap();
    assert!(serialized.contains("mine_iron"));
    assert!(!serialized.contains("Mine Iron"));
    let mut restored = BuildingProductionStore::default();
    restored.import_save_state(exported);
    assert_eq!(
        restored
            .get_policy(building_id)
            .and_then(|policy| policy.selected_operation.clone()),
        Some(operation_id)
    );
}
