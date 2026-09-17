//! Unit Editor client events (CG3).

use bevy::prelude::*;

use crate::menu::AppScreen;
use crate::world::{AppearanceProfileCatalog, UnitCatalog, UnitId, WorldData};

/// Dev or future CG8 entry requests opening the editor for one live unit.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenUnitEditorRequest(pub UnitId);

pub fn consume_open_unit_editor_requests(
    mut commands: Commands,
    mut next_screen: ResMut<NextState<AppScreen>>,
    mut requests: MessageReader<OpenUnitEditorRequest>,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    profiles: Res<AppearanceProfileCatalog>,
) {
    for request in requests.read() {
        if let Err(message) = super::open_unit_editor_for_live_unit(
            &mut commands,
            &mut next_screen,
            request.0,
            &world,
            &catalog,
            &profiles,
        ) {
            warn!("unit editor open failed: {message}");
        }
    }
}
