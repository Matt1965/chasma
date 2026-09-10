//! Per-instance GLB material preparation (ADR-095 BA1).
//!
//! Clones scene materials before styling so ghost/lifecycle passes never mutate shared
//! glTF asset materials (which would break other instances and dev reloads).
//!
//! Each mesh remembers its pristine GLB [`StandardMaterial`] handle on first preparation.
//! Every lifecycle refresh clones from that source, never from a prior lifecycle clone.

use bevy::prelude::*;

use crate::world::{Affiliation, BuildingLifecycleState};

use super::components::OriginalBuildingMaterial;
use super::placeholder::lifecycle_building_color;

/// Clone and style mesh materials for one scene hierarchy.
pub fn prepare_scene_materials(
    commands: &mut Commands,
    entity: Entity,
    children: &Query<&Children>,
    mesh_materials: &Query<&MeshMaterial3d<StandardMaterial>>,
    originals: &Query<&OriginalBuildingMaterial>,
    materials: &mut Assets<StandardMaterial>,
    lifecycle: BuildingLifecycleState,
    affiliation: Affiliation,
) -> bool {
    let mut touched = false;
    if let Ok(mesh_material) = mesh_materials.get(entity) {
        let original_handle = if let Ok(original) = originals.get(entity) {
            original.handle.clone()
        } else {
            let handle = mesh_material.0.clone();
            commands
                .entity(entity)
                .insert(OriginalBuildingMaterial::new(handle.clone()));
            handle
        };
        let mut cloned = materials.get(&original_handle).cloned().unwrap_or_default();
        apply_lifecycle_material_style(&mut cloned, lifecycle, affiliation);
        let handle = materials.add(cloned);
        commands.entity(entity).insert(MeshMaterial3d(handle));
        touched = true;
    }
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            touched |= prepare_scene_materials(
                commands,
                child,
                children,
                mesh_materials,
                originals,
                materials,
                lifecycle,
                affiliation,
            );
        }
    }
    touched
}

fn apply_lifecycle_material_style(
    material: &mut StandardMaterial,
    lifecycle: BuildingLifecycleState,
    affiliation: Affiliation,
) {
    match lifecycle {
        BuildingLifecycleState::Complete => {
            // `material` is already a fresh clone of the authored GLB source. Preserve lit
            // PBR fields, textures, and alpha unless a nearly-opaque blend mode needs
            // normalization for the renderer.
            if material.alpha_mode == AlphaMode::Blend && material.base_color.alpha() >= 0.99 {
                material.alpha_mode = AlphaMode::Opaque;
            }
        }
        _ => {
            material.unlit = true;
            let color = lifecycle_building_color(lifecycle, affiliation);
            material.base_color = color;
            material.alpha_mode = if color.to_srgba().alpha < 0.99 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::{App, MinimalPlugins};

    use crate::world::Affiliation;

    fn sample_source_material(
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        materials.add(StandardMaterial {
            base_color: Color::srgba(0.62, 0.41, 0.22, 1.0),
            metallic: 0.2,
            perceptual_roughness: 0.7,
            unlit: false,
            alpha_mode: AlphaMode::Opaque,
            ..default()
        })
    }

    #[test]
    fn complete_restores_pristine_glb_material_not_prior_tint() {
        let mut materials = Assets::<StandardMaterial>::default();
        let source = sample_source_material(&mut materials);
        let mut tinted = materials.get(&source).cloned().expect("source");
        apply_lifecycle_material_style(
            &mut tinted,
            BuildingLifecycleState::Planned,
            Affiliation::Player,
        );
        assert!(tinted.unlit);
        assert!(tinted.base_color.alpha() < 1.0);

        let mut restored = materials.get(&source).cloned().expect("source");
        apply_lifecycle_material_style(
            &mut restored,
            BuildingLifecycleState::Complete,
            Affiliation::Player,
        );
        assert!(!restored.unlit);
        let source_material = materials.get(&source).expect("source");
        assert_eq!(restored.base_color, source_material.base_color);
        assert_eq!(restored.metallic, source_material.metallic);
        assert_eq!(restored.alpha_mode, AlphaMode::Opaque);
    }

    #[test]
    fn planned_lifecycle_applies_translucent_tint_from_source() {
        let mut materials = Assets::<StandardMaterial>::default();
        let source = sample_source_material(&mut materials);
        let mut planned = materials.get(&source).cloned().expect("source");
        apply_lifecycle_material_style(
            &mut planned,
            BuildingLifecycleState::Planned,
            Affiliation::Player,
        );
        assert!(planned.unlit);
        assert_eq!(planned.alpha_mode, AlphaMode::Blend);
        assert_ne!(
            planned.base_color,
            materials.get(&source).unwrap().base_color
        );
    }

    #[test]
    fn lifecycle_refresh_remembers_original_handle_per_mesh() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<StandardMaterial>();

        let source = {
            let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
            sample_source_material(&mut materials)
        };
        let mesh = app
            .world_mut()
            .spawn((MeshMaterial3d(source.clone()),))
            .id();

        let lifecycle = BuildingLifecycleState::Planned;
        let affiliation = Affiliation::Player;
        app.world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      children: Query<&Children>,
                      mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
                      originals: Query<&OriginalBuildingMaterial>,
                      mut materials: ResMut<Assets<StandardMaterial>>| {
                    prepare_scene_materials(
                        &mut commands,
                        mesh,
                        &children,
                        &mesh_materials,
                        &originals,
                        &mut materials,
                        lifecycle,
                        affiliation,
                    );
                },
            )
            .expect("planned tint");

        let planned_handle = app
            .world_mut()
            .get::<MeshMaterial3d<StandardMaterial>>(mesh)
            .expect("mesh material")
            .0
            .clone();
        let planned = app
            .world_mut()
            .resource::<Assets<StandardMaterial>>()
            .get(&planned_handle)
            .expect("planned material");
        assert!(planned.unlit);
        assert_eq!(
            app.world_mut()
                .get::<OriginalBuildingMaterial>(mesh)
                .expect("original marker")
                .handle,
            source
        );

        let lifecycle = BuildingLifecycleState::Complete;
        app.world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      children: Query<&Children>,
                      mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
                      originals: Query<&OriginalBuildingMaterial>,
                      mut materials: ResMut<Assets<StandardMaterial>>| {
                    prepare_scene_materials(
                        &mut commands,
                        mesh,
                        &children,
                        &mesh_materials,
                        &originals,
                        &mut materials,
                        lifecycle,
                        affiliation,
                    );
                },
            )
            .expect("complete restore");

        let (complete_unlit, complete_base_color, source_base_color, complete_handle) = {
            let world = app.world_mut();
            let complete_handle = world
                .get::<MeshMaterial3d<StandardMaterial>>(mesh)
                .expect("mesh material")
                .0
                .clone();
            let materials = world.resource::<Assets<StandardMaterial>>();
            let complete = materials.get(&complete_handle).expect("complete material");
            let source_material = materials.get(&source).expect("source");
            (
                complete.unlit,
                complete.base_color,
                source_material.base_color,
                complete_handle,
            )
        };
        assert!(!complete_unlit);
        assert_eq!(complete_base_color, source_base_color);
        assert_ne!(complete_handle, planned_handle);
    }
}
