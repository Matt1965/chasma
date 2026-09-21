//! Appearance morph presentation sync (CG2).

mod components;
pub(crate) mod sync;

#[cfg(test)]
mod tests;

pub use components::UnitAppearanceMorphFingerprint;
pub use sync::sync_unit_appearance_morphs;
