//! Test hooks for dev scene apply (not used in production).

use std::sync::atomic::{AtomicBool, Ordering};

static INJECT_LATE_APPLY_FAILURE: AtomicBool = AtomicBool::new(false);

/// When enabled, [`super::load::apply_scene_plan`] fails after restore/rebuild steps.
pub fn set_inject_late_apply_failure(enable: bool) {
    INJECT_LATE_APPLY_FAILURE.store(enable, Ordering::SeqCst);
}

pub(crate) fn late_apply_failure_active() -> bool {
    INJECT_LATE_APPLY_FAILURE.load(Ordering::SeqCst)
}
