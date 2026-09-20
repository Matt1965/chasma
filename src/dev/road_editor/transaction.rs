use crate::world::{Road, RoadId, RoadNetwork, refresh_all_tee_branches};

use super::state::RoadEditMode;

/// Snapshot captured when entering a transactional road tool.
#[derive(Debug, Clone, PartialEq)]
pub struct RoadToolTransaction {
    pub dirty_before: bool,
    pub kind: RoadToolTransactionKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RoadToolTransactionKind {
    Create,
    Extend {
        road_id: RoadId,
        original_road: Road,
    },
    InsertPoint,
}

pub fn rollback_transaction(network: &mut RoadNetwork, transaction: &RoadToolTransaction) {
    match &transaction.kind {
        RoadToolTransactionKind::Create => {}
        RoadToolTransactionKind::Extend {
            road_id,
            original_road,
        } => {
            network.roads.insert(road_id.clone(), original_road.clone());
            refresh_all_tee_branches(network);
        }
        RoadToolTransactionKind::InsertPoint => {}
    }
}

pub fn is_modal_road_tool(mode: RoadEditMode) -> bool {
    matches!(
        mode,
        RoadEditMode::Create
            | RoadEditMode::ExtendStart
            | RoadEditMode::ExtendEnd
            | RoadEditMode::InsertPoint
    )
}
