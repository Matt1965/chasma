use bevy::prelude::*;

use crate::buildings::{BuildingRuntimeSystems, BuildingsRuntimePlugin};
use crate::camera::{CameraControlSystems, CameraPlugin};
use crate::doodads::{DoodadRuntimeSystems, DoodadsRuntimePlugin};
use crate::environment::EnvironmentPlugin;
use crate::item_piles::{ItemPileRuntimePlugin, ItemPileRuntimeSystems};
use crate::player::{PlayerControlSystems, PlayerPlugin, RuntimeSyncSystems};
use crate::projectiles::{ProjectileRuntimeSystems, ProjectilesRuntimePlugin};
use crate::simulation::{SimulationControlSystems, SimulationSystems};
use crate::terrain::{TerrainRuntimePlugin, TerrainStreamingSystems};
use crate::units::{UnitAnimationSystems, UnitRuntimeSystems, UnitsRuntimePlugin};
use crate::view::ViewPlugin;
use crate::world::WorldFoundationPlugin;

mod view_focus;

pub use view_focus::publish_primary_view_focus;

/// Composition root for the application.
///
/// `AppPlugin` is the single place where architectural layer plugins are
/// registered, in dependency order. It owns wiring only: no data and no
/// systems. Additional layers (occupancy, gameplay, simulation) register here
/// as they gain real content. See ADR-007.
pub struct AppPlugin;

/// Bridges camera state into generic view presentation (ADR-014).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct ViewFocusSystems;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ViewPlugin)
            .add_plugins(WorldFoundationPlugin)
            .add_plugins(TerrainRuntimePlugin)
            .add_plugins(DoodadsRuntimePlugin)
            .add_plugins(ItemPileRuntimePlugin)
            .add_plugins(BuildingsRuntimePlugin)
            .add_plugins(UnitsRuntimePlugin)
            .add_plugins(ProjectilesRuntimePlugin)
            .add_plugins(PlayerPlugin)
            .add_plugins(EnvironmentPlugin)
            .add_plugins(CameraPlugin)
            .configure_sets(Update, ViewFocusSystems.after(CameraControlSystems))
            .configure_sets(Update, TerrainStreamingSystems.after(ViewFocusSystems))
            .configure_sets(
                Update,
                DoodadRuntimeSystems
                    .after(TerrainStreamingSystems)
                    .in_set(RuntimeSyncSystems),
            )
            .configure_sets(
                Update,
                ItemPileRuntimeSystems
                    .after(DoodadRuntimeSystems)
                    .in_set(RuntimeSyncSystems),
            )
            .configure_sets(
                Update,
                BuildingRuntimeSystems
                    .after(ItemPileRuntimeSystems)
                    .in_set(RuntimeSyncSystems),
            )
            .configure_sets(
                Update,
                UnitRuntimeSystems
                    .after(BuildingRuntimeSystems)
                    .in_set(RuntimeSyncSystems),
            )
            .configure_sets(
                Update,
                UnitAnimationSystems
                    .after(UnitRuntimeSystems)
                    .after(SimulationSystems),
            )
            .configure_sets(
                Update,
                ProjectileRuntimeSystems
                    .after(UnitRuntimeSystems)
                    .in_set(RuntimeSyncSystems),
            )
            .configure_sets(Update, PlayerControlSystems.after(RuntimeSyncSystems))
            .configure_sets(Update, SimulationControlSystems.before(SimulationSystems))
            .add_systems(Update, publish_primary_view_focus.in_set(ViewFocusSystems));

        #[cfg(feature = "dev")]
        {
            app.add_plugins(crate::terrain::preview::TerrainPreviewPlugin);
        }
    }
}
