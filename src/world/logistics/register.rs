use crate::world::building::catalog::BuildingDefinition;
use crate::world::building::inventory_binding::effective_inventory_binding_definitions;
use crate::world::{BuildingId, WorldData, cancel_hauling_request};
/// Register a building's inventory endpoints in the logistics index (EP7).
pub fn register_building_logistics_endpoints(
    world: &mut WorldData,
    definition: &BuildingDefinition,
    building_id: BuildingId,
) {
    for binding in effective_inventory_binding_definitions(definition) {
        world.logistics_endpoint_index_mut().register(
            &definition.id,
            &binding.binding_id,
            building_id,
        );
    }
}

/// Unregister a building from the logistics index (EP7).
pub fn unregister_building_logistics_endpoints(
    world: &mut WorldData,
    definition: &BuildingDefinition,
    building_id: BuildingId,
) {
    let binding_ids: Vec<_> = effective_inventory_binding_definitions(definition)
        .into_iter()
        .map(|binding| binding.binding_id)
        .collect();
    world.logistics_endpoint_index_mut().unregister_building(
        &definition.id,
        &binding_ids,
        building_id,
    );
}

/// Cancel hauling state owned by a removed building (EP7).
pub fn cancel_logistics_for_building_removal(
    world: &mut WorldData,
    building_id: BuildingId,
) {
    let cancelled = world
        .hauling_request_store_mut()
        .cancel_requests_for_building(building_id);
    for request_id in cancelled {
        cancel_hauling_request(world, request_id);
    }
}
