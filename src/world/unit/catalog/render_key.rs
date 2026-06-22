use bevy::prelude::*;

/// Future rendering asset lookup key for unit glTF scenes (ADR-027, ADR-028).
///
/// Excel import stores bare asset stems (`wolf`) via
/// [`crate::data_import::unit::normalize_file_path_to_render_key`]. Runtime
/// resolves `assets/units/{key}.glb`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Reflect)]
pub struct UnitRenderKey(pub Option<String>);

impl UnitRenderKey {
    pub fn unset() -> Self {
        Self(None)
    }

    pub fn reserved(key: impl Into<String>) -> Self {
        Self(Some(key.into()))
    }
}
