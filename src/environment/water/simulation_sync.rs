//! Keep Environment water presentation observing gameplay water authority.

use bevy::prelude::*;

use crate::environment::WaterSettings;
use crate::terrain::TerrainRenderAssets;
use crate::world::{WorldData, refresh_all_unit_locomotion};

/// Convert sim water into `WaterSettings` and bootstrap sim from presentation once.
pub fn sync_water_presentation_from_simulation(
    mut world: Option<ResMut<WorldData>>,
    mut settings: ResMut<WaterSettings>,
    render: Option<Res<TerrainRenderAssets>>,
    unit_catalog: Option<Res<crate::world::UnitCatalog>>,
    weapon_catalog: Option<Res<crate::world::WeaponCatalog>>,
) {
    let Some(world) = world.as_deref_mut() else {
        return;
    };
    let Some(render) = render else {
        return;
    };
    let scale = render.vertical_scale;
    let presentation_enabled = settings.enabled;
    {
        let water = world.water_mut();
        if water.has_presentation_seed() {
            water.apply_presentation_scale(scale);
            water.enabled = presentation_enabled;
        }
        settings.enabled = water.enabled;
        settings.water_level = water.presentation_surface_y(scale);
    }
    if let (Some(units), Some(weapons)) = (unit_catalog, weapon_catalog) {
        refresh_all_unit_locomotion(world, units.as_ref(), weapons.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use crate::world::{DEFAULT_PRESENTATION_WATER_LEVEL, presentation_to_sim, sim_to_presentation};

    #[test]
    fn sim_conversion_does_not_copy_presentation_y() {
        let scale = 8.0;
        let sim = presentation_to_sim(DEFAULT_PRESENTATION_WATER_LEVEL, scale);
        assert!((sim - 7.0).abs() < 1e-5);
        assert_ne!(sim, DEFAULT_PRESENTATION_WATER_LEVEL);
        assert!((sim_to_presentation(sim, scale) - DEFAULT_PRESENTATION_WATER_LEVEL).abs() < 1e-4);
    }
}
