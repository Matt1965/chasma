//! System parameters for animation playback (A6 — Bevy param limit).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::camera::{RtsCamera, RtsCameraState};
use crate::simulation::SimulationControlState;
use crate::units::input::SelectedUnits;
use crate::units::spawn::UnitRenderIndex;
use crate::world::{AnimationProfileCatalog, UnitCatalog, WeaponCatalog, WorldData};

use super::assets::UnitAnimationAssets;
use super::components::UnitAnimationStateIndex;
use super::lod::{AnimationLodSettings, AnimationPresentationFocus, AnimationPresentationMetrics};
use super::settings::UnitAnimationSettings;

#[derive(SystemParam)]
pub struct AnimationPlaybackParams<'w, 's> {
    pub time: Res<'w, Time>,
    pub control: Res<'w, SimulationControlState>,
    pub settings: Res<'w, UnitAnimationSettings>,
    pub lod_settings: Res<'w, AnimationLodSettings>,
    pub focus: Res<'w, AnimationPresentationFocus>,
    pub selection: Res<'w, SelectedUnits>,
    pub camera: Query<'w, 's, &'static RtsCameraState, With<RtsCamera>>,
    pub world: Res<'w, WorldData>,
    pub catalog: Res<'w, UnitCatalog>,
    pub weapons: Res<'w, WeaponCatalog>,
    pub profiles: Res<'w, AnimationProfileCatalog>,
    pub assets: Res<'w, UnitAnimationAssets>,
    pub index: Res<'w, UnitRenderIndex>,
    pub state_index: ResMut<'w, UnitAnimationStateIndex>,
    pub metrics: ResMut<'w, AnimationPresentationMetrics>,
}
