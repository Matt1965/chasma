//! Authoritative production runtime commands (EP2/EP3).

use crate::world::BuildingId;
use crate::world::WorldData;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::building::operation::{
    BuildingOperationPolicy, OperationDefinitionId, OperationLifecycle, RepeatMode,
};
use crate::world::operation::{OperationCatalog, OperationSelectionError, validate_operation_selection};

/// Production command failures (EP2/EP3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionCommandError {
    BuildingNotFound(BuildingId),
    InvalidRepeatCount,
    InvalidOperationSelection(OperationSelectionError),
}

impl std::fmt::Display for ProductionCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuildingNotFound(id) => write!(f, "building `{id:?}` not found"),
            Self::InvalidRepeatCount => write!(f, "repeat count must be greater than zero"),
            Self::InvalidOperationSelection(err) => write!(f, "{err}"),
        }
    }
}

fn require_building(world: &WorldData, building_id: BuildingId) -> Result<(), ProductionCommandError> {
    if world.get_building(building_id).is_none() {
        return Err(ProductionCommandError::BuildingNotFound(building_id));
    }
    Ok(())
}

pub fn set_production_enabled(
    world: &mut WorldData,
    building_id: BuildingId,
    enabled: bool,
) -> Result<(), ProductionCommandError> {
    require_building(world, building_id)?;
    let store = world.building_production_store_mut();
    store.get_policy_mut(building_id).enabled = enabled;
    if !enabled {
        let state = store.get_state_mut(building_id);
        state.lifecycle = OperationLifecycle::Disabled;
        state.blocked_reason = None;
        state.active_worker_count = 0;
    }
    Ok(())
}

pub fn set_production_paused(
    world: &mut WorldData,
    building_id: BuildingId,
    paused: bool,
) -> Result<(), ProductionCommandError> {
    require_building(world, building_id)?;
    let store = world.building_production_store_mut();
    store.get_policy_mut(building_id).paused = paused;
    if paused {
        let state = store.get_state_mut(building_id);
        state.lifecycle = OperationLifecycle::Paused;
        state.blocked_reason = None;
    }
    Ok(())
}

pub fn set_production_execution_mode(
    world: &mut WorldData,
    building_id: BuildingId,
    repeat_mode: RepeatMode,
) -> Result<(), ProductionCommandError> {
    if !repeat_mode.is_valid() {
        return Err(ProductionCommandError::InvalidRepeatCount);
    }
    require_building(world, building_id)?;
    world
        .building_production_store_mut()
        .get_policy_mut(building_id)
        .repeat_mode = repeat_mode;
    Ok(())
}

pub fn set_production_repeat_count(
    world: &mut WorldData,
    building_id: BuildingId,
    count: u32,
) -> Result<(), ProductionCommandError> {
    if count == 0 {
        return Err(ProductionCommandError::InvalidRepeatCount);
    }
    set_production_execution_mode(world, building_id, RepeatMode::Count(count))
}

pub fn set_production_selected_operation(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    building_id: BuildingId,
    operation: Option<OperationDefinitionId>,
) -> Result<(), ProductionCommandError> {
    require_building(world, building_id)?;
    let building_record = world.get_building(building_id).unwrap();
    let building_definition = building_catalog
        .get(&building_record.definition_id)
        .ok_or(ProductionCommandError::BuildingNotFound(building_id))?;
    if let Some(operation_id) = operation.as_ref() {
        validate_operation_selection(
            building_definition,
            building_id,
            operation_catalog,
            operation_id,
        )
        .map_err(ProductionCommandError::InvalidOperationSelection)?;
    }
    world
        .building_production_store_mut()
        .get_policy_mut(building_id)
        .selected_operation = operation;
    Ok(())
}

pub fn reset_production_progress(
    world: &mut WorldData,
    building_id: BuildingId,
) -> Result<(), ProductionCommandError> {
    require_building(world, building_id)?;
    world
        .building_production_store_mut()
        .reset_progress(building_id);
    Ok(())
}

pub fn production_policy(
    world: &WorldData,
    building_id: BuildingId,
) -> Option<BuildingOperationPolicy> {
    world
        .building_production_store()
        .get_policy(building_id)
        .cloned()
}

pub fn cycle_production_selected_operation(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    building_id: BuildingId,
    forward: bool,
) -> Result<Option<OperationDefinitionId>, ProductionCommandError> {
    require_building(world, building_id)?;
    let building_record = world.get_building(building_id).unwrap();
    let building_definition = building_catalog
        .get(&building_record.definition_id)
        .ok_or(ProductionCommandError::BuildingNotFound(building_id))?;
    let supported: Vec<_> = building_definition
        .supported_operations
        .iter()
        .filter(|operation_id| operation_catalog.get(operation_id).is_some())
        .cloned()
        .collect();
    if supported.is_empty() {
        return Ok(None);
    }
    let current = world
        .building_production_store()
        .get_policy(building_id)
        .and_then(|policy| policy.selected_operation.clone());
    let next = if let Some(current) = current {
        let index = supported
            .iter()
            .position(|id| *id == current)
            .unwrap_or(0);
        let next_index = if forward {
            (index + 1) % supported.len()
        } else {
            (index + supported.len() - 1) % supported.len()
        };
        supported[next_index].clone()
    } else {
        supported[0].clone()
    };
    set_production_selected_operation(
        world,
        building_catalog,
        operation_catalog,
        building_id,
        Some(next.clone()),
    )?;
    Ok(Some(next))
}
