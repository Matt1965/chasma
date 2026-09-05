//! Settlement workforce matrix presentation model.

use crate::client::CameraSettlementContext;
use crate::ui::gameplay::squad_panel::squad_display_name;
use crate::world::{
    SettlementId, UnitCatalog, UnitId, WorkPermissionDomain, WorkSkillCatalog, WorldData,
    unit_physically_capable_for_work_permission, unit_work_allowed,
    work_skill_for_permission_domain, work_skill_value,
};

/// One permission column cell for a settlement member row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkforceMatrixCell {
    pub domain: WorkPermissionDomain,
    pub skill_value: i64,
    pub permission_allowed: bool,
    /// `None` when capability architecture cannot classify the domain cleanly.
    pub physically_capable: Option<bool>,
}

/// One settlement member row in the workforce matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkforceMatrixRow {
    pub unit_id: UnitId,
    pub display_name: String,
    pub cells: Vec<WorkforceMatrixCell>,
}

/// Full workforce matrix snapshot for the focused settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementWorkforceSnapshot {
    pub settlement_id: Option<SettlementId>,
    pub title: String,
    pub empty_message: Option<String>,
    pub permission_columns: Vec<WorkPermissionDomain>,
    pub rows: Vec<WorkforceMatrixRow>,
}

pub const NO_FOCUSED_SETTLEMENT_MESSAGE: &str = "No focused settlement";
pub const NO_SETTLEMENT_WORKERS_MESSAGE: &str = "No settlement workers";

/// Authoritative settlement member ids for the workforce matrix (alive, sorted).
pub fn settlement_workforce_member_unit_ids(
    world: &WorldData,
    settlement_id: SettlementId,
) -> Vec<UnitId> {
    crate::world::settlement_member_unit_ids(world, settlement_id)
}

pub fn build_settlement_workforce_snapshot(
    context: &CameraSettlementContext,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    work_skill_catalog: &WorkSkillCatalog,
) -> SettlementWorkforceSnapshot {
    let permission_columns = WorkPermissionDomain::ALL.to_vec();
    let Some(settlement_id) = context.focused_settlement_id else {
        return SettlementWorkforceSnapshot {
            settlement_id: None,
            title: "Settlement Workforce".into(),
            empty_message: Some(NO_FOCUSED_SETTLEMENT_MESSAGE.into()),
            permission_columns,
            rows: Vec::new(),
        };
    };

    let settlement_name = world
        .settlement_store()
        .get_settlement(settlement_id)
        .map(|record| record.display_name.clone())
        .unwrap_or_else(|| format!("Settlement {}", settlement_id.raw()));

    let member_ids = settlement_workforce_member_unit_ids(world, settlement_id);
    if member_ids.is_empty() {
        return SettlementWorkforceSnapshot {
            settlement_id: Some(settlement_id),
            title: format!("Settlement Workforce — {}", settlement_name),
            empty_message: Some(NO_SETTLEMENT_WORKERS_MESSAGE.into()),
            permission_columns,
            rows: Vec::new(),
        };
    }

    let rows = member_ids
        .into_iter()
        .filter_map(|unit_id| {
            build_workforce_matrix_row(
                settlement_id,
                unit_id,
                world,
                unit_catalog,
                work_skill_catalog,
            )
        })
        .collect();

    SettlementWorkforceSnapshot {
        settlement_id: Some(settlement_id),
        title: format!("Settlement Workforce — {}", settlement_name),
        empty_message: None,
        permission_columns,
        rows,
    }
}

fn build_workforce_matrix_row(
    settlement_id: SettlementId,
    unit_id: UnitId,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    work_skill_catalog: &WorkSkillCatalog,
) -> Option<WorkforceMatrixRow> {
    if world.get_unit(unit_id).is_none() {
        return None;
    }
    let display_name = squad_display_name(unit_id, world, unit_catalog);
    let cells = WorkPermissionDomain::ALL
        .map(|domain| {
            let skill_id = work_skill_for_permission_domain(domain);
            let skill_value =
                work_skill_value(world, work_skill_catalog, unit_id, &skill_id).unwrap_or_default();
            WorkforceMatrixCell {
                domain,
                skill_value,
                permission_allowed: unit_work_allowed(world, settlement_id, unit_id, domain),
                physically_capable: unit_physically_capable_for_work_permission(
                    unit_catalog,
                    world,
                    unit_id,
                    domain,
                ),
            }
        })
        .to_vec();
    Some(WorkforceMatrixRow {
        unit_id,
        display_name,
        cells,
    })
}

pub fn permission_column_labels(snapshot: &SettlementWorkforceSnapshot) -> Vec<&'static str> {
    snapshot
        .permission_columns
        .iter()
        .map(|domain| domain.label())
        .collect()
}

pub fn snapshot_contains_permission_column(
    snapshot: &SettlementWorkforceSnapshot,
    domain: WorkPermissionDomain,
) -> bool {
    snapshot.permission_columns.contains(&domain)
}
