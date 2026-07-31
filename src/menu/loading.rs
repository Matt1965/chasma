//! Loading screen session and readiness seam (temporary bridge).

use bevy::prelude::*;

use super::screen::{AppScreen, GameSessionKind};
use crate::simulation::SimulationControlState;
use crate::world::WorldData;

/// Minimum Loading frames before InGame (all builds) so the screen can present.
pub const LOADING_MIN_FRAMES: u32 = 2;

/// Dev-only minimum wall-clock visibility so Loading is human-observable.
/// Applied only when compiling with `--features dev`; easy to change or remove.
pub const DEV_LOADING_MIN_VISIBLE_SECS: f32 = 0.25;

/// Frame budget while waiting for initial terrain residency (never hang forever).
pub const LOADING_TERRAIN_FRAME_BUDGET: u32 = 90;

/// Truthful loading phase label (no fake percentages).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadingPhase {
    #[default]
    Idle,
    PreparingNewGame,
    PreparingDefaultWorldEditor,
    PreparingTerrain,
    EnteringWorld,
}

impl LoadingPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::PreparingNewGame => "Preparing new game...",
            Self::PreparingDefaultWorldEditor => "Preparing default world editor...",
            Self::PreparingTerrain => "Preparing terrain...",
            Self::EnteringWorld => "Entering world...",
        }
    }
}

/// Client-local loading progress for the temporary session bridge.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct LoadingSession {
    pub phase: LoadingPhase,
    pub frames_visible: u32,
    pub target_kind: GameSessionKind,
    /// `Time::elapsed_secs()` when Loading became active (`None` until first tick).
    pub entered_at_secs: Option<f32>,
}

impl Default for LoadingSession {
    fn default() -> Self {
        Self {
            phase: LoadingPhase::Idle,
            frames_visible: 0,
            target_kind: GameSessionKind::None,
            entered_at_secs: None,
        }
    }
}

impl LoadingSession {
    pub fn begin(&mut self, kind: GameSessionKind) {
        self.target_kind = kind;
        self.frames_visible = 0;
        self.entered_at_secs = None;
        self.phase = match kind {
            GameSessionKind::DefaultWorldAuthoring => LoadingPhase::PreparingDefaultWorldEditor,
            _ => LoadingPhase::PreparingNewGame,
        };
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Pure readiness gate (testable without a full Bevy app).
///
/// `dev_min_visible_secs`: `Some(0.25)` in Dev builds, `None` in normal builds.
pub fn loading_bridge_may_enter(
    frames_visible: u32,
    elapsed_secs: f32,
    terrain_waiting: bool,
    dev_min_visible_secs: Option<f32>,
) -> bool {
    if frames_visible < LOADING_MIN_FRAMES {
        return false;
    }
    if let Some(min_secs) = dev_min_visible_secs {
        if elapsed_secs < min_secs {
            return false;
        }
    }
    if terrain_waiting && frames_visible < LOADING_TERRAIN_FRAME_BUDGET {
        return false;
    }
    true
}

/// Advance loading labels and enter InGame when the temporary bridge is ready.
pub fn tick_loading_session(
    time: Res<Time>,
    screen: Res<State<AppScreen>>,
    mut loading: ResMut<LoadingSession>,
    mut next_screen: ResMut<NextState<AppScreen>>,
    mut control: ResMut<SimulationControlState>,
    world: Res<WorldData>,
    terrain_catalog: Option<Res<crate::terrain::TerrainWorldCatalog>>,
) {
    if *screen.get() != AppScreen::Loading {
        return;
    }
    control.pause();
    loading.frames_visible = loading.frames_visible.saturating_add(1);
    if loading.entered_at_secs.is_none() {
        loading.entered_at_secs = Some(time.elapsed_secs());
    }
    let elapsed = time.elapsed_secs() - loading.entered_at_secs.unwrap_or(time.elapsed_secs());

    let terrain_waiting = terrain_catalog.is_some() && world.is_empty();
    if terrain_waiting {
        loading.phase = LoadingPhase::PreparingTerrain;
    }

    #[cfg(feature = "dev")]
    let dev_min = Some(DEV_LOADING_MIN_VISIBLE_SECS);
    #[cfg(not(feature = "dev"))]
    let dev_min = None;

    if !loading_bridge_may_enter(loading.frames_visible, elapsed, terrain_waiting, dev_min) {
        return;
    }

    loading.phase = LoadingPhase::EnteringWorld;
    loading.clear();
    next_screen.set(AppScreen::InGame);
}
