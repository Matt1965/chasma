//! Blueprint authority classification (NV1.5).

use super::catalog::BuildingNavigationBlueprintCatalog;
use super::definition::BuildingNavigationBlueprintInstanceOverride;
use crate::world::building::catalog::BuildingDefinition;

/// Which layer owns the effective navigation blueprint for a building instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlueprintAuthoritySource {
    InstanceOverride,
    AssetDefault,
    Generated,
    None,
}

impl BlueprintAuthoritySource {
    pub fn label(self) -> &'static str {
        match self {
            Self::InstanceOverride => "Instance Override",
            Self::AssetDefault => "Asset Default",
            Self::Generated => "Generated",
            Self::None => "None",
        }
    }
}

/// Classify persisted blueprint ownership without considering unsaved editor state.
pub fn classify_blueprint_authority(
    definition: &BuildingDefinition,
    catalog: &BuildingNavigationBlueprintCatalog,
    instance_override: Option<&BuildingNavigationBlueprintInstanceOverride>,
) -> BlueprintAuthoritySource {
    if let Some(override_data) = instance_override {
        if override_data.inline_blueprint.is_some() || override_data.blueprint_id.is_some() {
            return BlueprintAuthoritySource::InstanceOverride;
        }
    }
    if definition.navigation_blueprint_id.is_some() {
        return BlueprintAuthoritySource::AssetDefault;
    }
    #[cfg(feature = "data-import")]
    {
        use super::generate::blueprint_id_for_building;
        let generated_id = blueprint_id_for_building(definition);
        if catalog.get(&generated_id).is_some() {
            return BlueprintAuthoritySource::Generated;
        }
    }
    BlueprintAuthoritySource::None
}
