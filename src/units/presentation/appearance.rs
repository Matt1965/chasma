//! Copy authoritative appearance onto live render roots for presentation consumers.

use bevy::prelude::*;

use crate::units::components::{UnitRenderEntity, UnitSceneRoot};
use crate::units::UnitAppearanceMorphFingerprint;
use crate::units::spawn::UnitRenderIndex;
use crate::world::{UnitAppearance, WorldData};

/// Client-local appearance source for presentation systems (morphs, render-key resolution).
///
/// Live units receive a copy from [`WorldData`] each frame. Editor preview roots set this
/// directly from a draft without creating a gameplay [`UnitRecord`].
#[derive(Component, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct UnitPresentationAppearance {
    pub appearance: UnitAppearance,
}

/// Mirror authoritative unit appearance onto rendered gameplay units.
pub fn sync_live_unit_presentation_appearance(
    mut commands: Commands,
    world: Res<WorldData>,
    index: Res<UnitRenderIndex>,
    roots: Query<(Entity, &UnitRenderEntity), With<UnitSceneRoot>>,
    existing: Query<&UnitPresentationAppearance>,
) {
    for (entity, marker) in &roots {
        if index.0.get(&marker.unit_id) != Some(&entity) {
            continue;
        }
        let Some(unit) = world.get_unit(marker.unit_id) else {
            commands.entity(entity).remove::<UnitPresentationAppearance>();
            continue;
        };
        let Some(appearance) = unit.appearance.as_ref() else {
            commands.entity(entity).remove::<UnitPresentationAppearance>();
            continue;
        };
        if existing.get(entity).is_ok_and(|value| value.appearance == *appearance) {
            continue;
        }
        commands.entity(entity).insert(UnitPresentationAppearance {
            appearance: appearance.clone(),
        });
        commands.entity(entity).remove::<UnitAppearanceMorphFingerprint>();
    }
}
