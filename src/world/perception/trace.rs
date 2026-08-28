//! Dev-only perception trace lines (ADR-132 Phase 4).

use crate::world::unit::UnitId;

#[inline]
pub fn perception_query(observer: UnitId, sight_range_meters: f32, candidates: &[UnitId]) {
    #[cfg(feature = "dev")]
    crate::logging::write_perception_trace(format!(
        "observer={} sight_range_m={:.2} candidates={}",
        observer.0,
        sight_range_meters,
        candidates
            .iter()
            .map(|id| id.raw().to_string())
            .collect::<Vec<_>>()
            .join(",")
    ));
}
