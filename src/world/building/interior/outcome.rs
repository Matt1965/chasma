//! Per-building interior activation outcome (IN-11b).
//!
//! Interior activation used to fail silently: a building without an
//! `interior_profile_id` returned `Ok(())` having created nothing, so a building
//! with a valid persisted navigation blueprint and zero runtime spaces was
//! indistinguishable from a building that was never meant to have an interior.
//! This records why activation did what it did so persisted-versus-runtime
//! divergence is readable instead of inferred.

use std::collections::BTreeMap;

use super::id::InteriorProfileId;
use crate::world::BuildingId;
use crate::world::building::catalog::BuildingDefinitionId;
use crate::world::building::navigation_blueprint::{
    BlueprintAuthoritySource, BuildingNavigationBlueprintId,
};

/// What interior activation accomplished for one building instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteriorActivationStatus {
    /// Navigation blueprint and interior profile both activated.
    NavigationAndProfile,
    /// Blueprint activated; the definition declares no interior profile.
    NavigationWithoutProfile,
    /// Blueprint activated; a profile was named but could not be resolved.
    NavigationProfileMissing { profile_key: String },
    /// Profile-only interior (legacy presentation path), no navigation blueprint.
    ProfileWithoutNavigation,
    /// Nothing to activate: no blueprint and no profile.
    NoBlueprintNoProfile,
    /// A blueprint was referenced but could not be resolved.
    BlueprintResolutionFailed { reason: String },
    /// A blueprint resolved but failed validation.
    BlueprintValidationFailed { reason: String },
    /// Runtime space/portal registration failed.
    RuntimeRegistrationFailed { reason: String },
    /// Activation was requested for an already-active interior.
    AlreadyActivated,
    /// Runtime navigation was rebuilt for an already-active interior.
    Refreshed,
}

impl InteriorActivationStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NavigationAndProfile => "navigation + profile active",
            Self::NavigationWithoutProfile => "navigation active without profile",
            Self::NavigationProfileMissing { .. } => "navigation active, profile missing",
            Self::ProfileWithoutNavigation => "profile active, no navigation blueprint",
            Self::NoBlueprintNoProfile => "skipped: no blueprint and no profile",
            Self::BlueprintResolutionFailed { .. } => "blueprint resolution failed",
            Self::BlueprintValidationFailed { .. } => "blueprint validation failed",
            Self::RuntimeRegistrationFailed { .. } => "runtime registration failed",
            Self::AlreadyActivated => "already activated",
            Self::Refreshed => "refreshed",
        }
    }

    /// Whether runtime navigation spaces/portals exist as a result.
    pub fn navigation_active(&self) -> bool {
        matches!(
            self,
            Self::NavigationAndProfile
                | Self::NavigationWithoutProfile
                | Self::NavigationProfileMissing { .. }
                | Self::Refreshed
        )
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::NavigationProfileMissing { profile_key } => Some(profile_key),
            Self::BlueprintResolutionFailed { reason }
            | Self::BlueprintValidationFailed { reason }
            | Self::RuntimeRegistrationFailed { reason } => Some(reason),
            _ => None,
        }
    }
}

/// Latest activation result for one building, readable from dev diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteriorActivationOutcome {
    pub building_id: BuildingId,
    pub definition_id: BuildingDefinitionId,
    pub status: InteriorActivationStatus,
    pub blueprint_authority: BlueprintAuthoritySource,
    pub blueprint_id: Option<BuildingNavigationBlueprintId>,
    pub profile_id: Option<InteriorProfileId>,
    pub runtime_floor_count: usize,
    pub runtime_region_count: usize,
    pub runtime_portal_count: usize,
}

impl InteriorActivationOutcome {
    pub fn new(
        building_id: BuildingId,
        definition_id: BuildingDefinitionId,
        status: InteriorActivationStatus,
    ) -> Self {
        Self {
            building_id,
            definition_id,
            status,
            blueprint_authority: BlueprintAuthoritySource::None,
            blueprint_id: None,
            profile_id: None,
            runtime_floor_count: 0,
            runtime_region_count: 0,
            runtime_portal_count: 0,
        }
    }

    /// One-line dev summary, e.g.
    /// `hut #3: navigation active without profile — 1 floor, 1 region, 1 portal`.
    pub fn summary(&self) -> String {
        let mut text = format!(
            "{} #{}: {} — {} floor(s), {} region(s), {} portal(s)",
            self.definition_id.as_str(),
            self.building_id.raw(),
            self.status.label(),
            self.runtime_floor_count,
            self.runtime_region_count,
            self.runtime_portal_count,
        );
        if let Some(id) = &self.blueprint_id {
            text.push_str(&format!(
                " [{} via {}]",
                id.as_str(),
                self.blueprint_authority.label()
            ));
        }
        if let Some(reason) = self.status.reason() {
            text.push_str(&format!(" ({reason})"));
        }
        text
    }
}

/// Bounded store of the latest activation outcome per building.
///
/// One entry per building instance rather than a growing log, so this cannot
/// become per-frame spam.
#[derive(Debug, Clone, Default)]
pub struct InteriorActivationOutcomeStore {
    entries: BTreeMap<u64, InteriorActivationOutcome>,
}

impl InteriorActivationOutcomeStore {
    pub fn record(&mut self, outcome: InteriorActivationOutcome) {
        self.entries.insert(outcome.building_id.raw(), outcome);
    }

    pub fn get(&self, building_id: BuildingId) -> Option<&InteriorActivationOutcome> {
        self.entries.get(&building_id.raw())
    }

    pub fn remove(&mut self, building_id: BuildingId) {
        self.entries.remove(&building_id.raw());
    }

    pub fn iter(&self) -> impl Iterator<Item = &InteriorActivationOutcome> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
