//! CG2 Bevy 0.18 morph component spike.

use bevy::mesh::morph::MorphWeights;
use bevy::prelude::*;

use super::sync::apply_mesh_morph_weights;

#[test]
fn morph_weights_supports_independent_instances() {
    let mut weights_a =
        MorphWeights::new(vec![0.0, 0.8, 0.0, 0.0, 0.0, 0.0], None).expect("weights a");
    let mut weights_b =
        MorphWeights::new(vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.2], None).expect("weights b");
    weights_a.weights_mut()[1] = 0.9;
    weights_b.weights_mut()[5] = 0.4;
    assert!((weights_a.weights()[1] - 0.9).abs() < 1e-4);
    assert!((weights_b.weights()[5] - 0.4).abs() < 1e-4);
    assert_ne!(weights_a.weights()[1], weights_b.weights()[1]);
}

#[test]
fn apply_mesh_morph_weights_writes_rendered_primitive_weights() {
    use bevy::ecs::system::RunSystemOnce;

    let mut world = World::new();
    let entity = world
        .spawn(bevy::mesh::morph::MeshMorphWeights::new(vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap())
        .id();
    let weights = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    world
        .run_system_once(move |mut query: Query<&mut bevy::mesh::morph::MeshMorphWeights>| {
            apply_mesh_morph_weights(&mut query, entity, &weights)
        })
        .expect("system");
    let component = world.entity(entity).get::<bevy::mesh::morph::MeshMorphWeights>().unwrap();
    assert!((component.weights()[0] - 1.0).abs() < 1e-4);
    assert!(component.weights()[1..].iter().all(|value| *value < 1e-4));
}

#[test]
fn human_glb_bevy_loaded_body_mesh_exposes_morph_target_names() {
    use std::path::PathBuf;

    use bevy::asset::LoadState;
    use bevy::gltf::{Gltf, GltfMesh};
    use crate::world::HUMAN_MORPH_TARGET_NAMES;

    let asset_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: asset_root.to_string_lossy().into_owned(),
            ..default()
        },
        bevy::image::ImagePlugin::default(),
        bevy::mesh::MeshPlugin,
        bevy::scene::ScenePlugin,
        bevy::animation::AnimationPlugin,
        bevy::gltf::GltfPlugin::default(),
    ));
    app.init_asset::<bevy::pbr::StandardMaterial>();
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    app.finish();
    app.cleanup();

    let gltf_handle = app
        .world()
        .resource::<AssetServer>()
        .load("units/human_male.glb");
    let gltf_id = gltf_handle.id();

    let mut last_state = None;
    let mut loaded = false;
    for _ in 0..500 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let asset_server = app.world().resource::<AssetServer>();
        last_state = asset_server.get_load_state(gltf_id);
        if let Some(LoadState::Failed(err)) = last_state {
            panic!("human_male.glb load failed: {err}");
        }
        if asset_server.is_loaded_with_dependencies(gltf_id)
            && app.world().resource::<Assets<Gltf>>().get(&gltf_handle).is_some()
        {
            loaded = true;
            break;
        }
    }
    assert!(
        loaded,
        "timed out waiting for Bevy-loaded human_male.glb (last load state: {:?})",
        last_state,
    );

    let gltf = app
        .world()
        .resource::<Assets<Gltf>>()
        .get(&gltf_handle)
        .expect("gltf asset");
    let gltf_meshes = app.world().resource::<Assets<GltfMesh>>();
    let meshes = app.world().resource::<Assets<Mesh>>();

    let mut body_mesh: Option<(&str, &Mesh)> = None;
    for gltf_mesh_handle in &gltf.meshes {
        let gltf_mesh = gltf_meshes.get(gltf_mesh_handle).expect("gltf mesh");
        let lower = gltf_mesh.name.to_ascii_lowercase();
        if lower.contains("eye") || lower.contains("brow") || lower.contains("face") {
            continue;
        }
        for primitive in &gltf_mesh.primitives {
            let mesh = meshes.get(&primitive.mesh).expect("bevy mesh");
            if mesh.morph_targets().is_none() {
                continue;
            }
            if body_mesh.is_none_or(|(_, current)| mesh.count_vertices() > current.count_vertices()) {
                body_mesh = Some((gltf_mesh.name.as_str(), mesh));
            }
        }
    }
    let (mesh_name, body_mesh) = body_mesh.expect("Bevy-loaded body morph mesh");
    assert!(
        mesh_name.contains("Sphere.005_Retopology.004") || mesh_name.contains("Retopology"),
        "unexpected body mesh name: {mesh_name}",
    );
    let target_names = body_mesh
        .morph_target_names()
        .expect("Bevy-loaded body mesh must expose morph_target_names from mesh extras");
    assert_eq!(target_names.len(), HUMAN_MORPH_TARGET_NAMES.len());
    for (index, expected) in HUMAN_MORPH_TARGET_NAMES.iter().enumerate() {
        assert_eq!(target_names[index], *expected);
    }
    assert!(
        body_mesh.morph_targets().is_some(),
        "Bevy-loaded body mesh must carry morph target image data",
    );
    assert!(
        body_mesh.count_vertices() > 5_000,
        "body mesh vertex count unexpectedly low",
    );
}

#[test]
fn production_morph_sync_writes_build_broad_for_max_build() {
    use std::collections::BTreeMap;

    use bevy::ecs::system::RunSystemOnce;
    use bevy::mesh::morph::{MeshMorphWeights, MorphWeights};

    use crate::units::components::UnitSceneRoot;
    use crate::units::presentation::UnitPresentationAppearance;
    use crate::world::{
        AppearanceParamId, AppearanceParameterDefinition, AppearanceProfile,
        AppearanceProfileCatalog, AppearanceProfileId, BodyVariantDefinition, BodyVariantId,
        HUMAN_MORPH_TARGET_NAMES, MorphMappingSide, MorphTargetMapping, SpeciesId, UnitAppearance,
        UnitRenderKey,
    };

    use super::sync::sync_unit_appearance_morphs;

    let profile = AppearanceProfile {
        id: AppearanceProfileId::new("human"),
        species_id: SpeciesId::new("human"),
        schema_version: 1,
        height_scale_min: 0.85,
        height_scale_max: 1.15,
        height_scale_default: 1.0,
        body_variants: vec![BodyVariantDefinition {
            id: BodyVariantId::new("human_male"),
            display_name: "Male".into(),
            render_key: UnitRenderKey::reserved("human_male"),
            enabled: true,
        }],
        parameters: vec![AppearanceParameterDefinition {
            id: AppearanceParamId::new("build"),
            display_name: "Build".into(),
            category: "Body".into(),
            min: 0.0,
            max: 1.0,
            default: 0.5,
            display_order: 1,
            enabled: true,
        }],
        morph_mappings: vec![
            MorphTargetMapping {
                variant_id: BodyVariantId::new("human_male"),
                param_id: AppearanceParamId::new("build"),
                technical_target: "build_broad".into(),
                side: MorphMappingSide::AboveDefault,
                multiplier: 1.0,
                enabled: true,
            },
            MorphTargetMapping {
                variant_id: BodyVariantId::new("human_male"),
                param_id: AppearanceParamId::new("build"),
                technical_target: "build_narrow".into(),
                side: MorphMappingSide::BelowDefault,
                multiplier: 1.0,
                enabled: true,
            },
        ],
        enabled: true,
    };
    let catalog =
        AppearanceProfileCatalog::from_definitions(vec![profile]).expect("appearance catalog");

    let mut morphs = BTreeMap::new();
    morphs.insert(AppearanceParamId::new("build"), 1.0);
    let appearance = UnitAppearance {
        profile_id: AppearanceProfileId::new("human"),
        body_variant_id: BodyVariantId::new("human_male"),
        height_scale: 1.0,
        morphs,
        generation_seed: None,
    };

    let mut world = World::new();
    world.init_resource::<Assets<Mesh>>();
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0, 0.0, 0.0]]);
    mesh.set_morph_target_names(
        HUMAN_MORPH_TARGET_NAMES
            .iter()
            .map(|name| name.to_string())
            .collect(),
    );
    let mesh_handle = world.resource_mut::<Assets<Mesh>>().add(mesh);

    let root = world
        .spawn((
            UnitSceneRoot,
            UnitPresentationAppearance { appearance },
        ))
        .id();
    let parent = world
        .spawn((
            MorphWeights::new(vec![0.0; HUMAN_MORPH_TARGET_NAMES.len()], None).unwrap(),
            ChildOf(root),
        ))
        .id();
    let primitive = world
        .spawn((
            Mesh3d(mesh_handle),
            MeshMorphWeights::new(vec![0.0; HUMAN_MORPH_TARGET_NAMES.len()]).unwrap(),
            ChildOf(parent),
        ))
        .id();

    world.insert_resource(catalog);

    world
        .run_system_once(sync_unit_appearance_morphs)
        .expect("morph sync");

    let weights = world
        .entity(primitive)
        .get::<MeshMorphWeights>()
        .expect("primitive weights")
        .weights();
    assert!(weights[0] > 0.9, "build_broad should be active at build max");
    assert!(weights[1] < 0.1, "build_narrow should be inactive at build max");
    assert!(
        world.entity(parent).get::<MorphWeights>().is_none(),
        "parent MorphWeights must be removed so inheritance cannot overwrite primitives",
    );
}

#[test]
fn production_morph_sync_writes_build_narrow_for_min_build() {
    use std::collections::BTreeMap;

    use bevy::ecs::system::RunSystemOnce;
    use bevy::mesh::morph::MeshMorphWeights;

    use crate::units::components::UnitSceneRoot;
    use crate::units::presentation::UnitPresentationAppearance;
    use crate::world::{
        AppearanceParamId, AppearanceParameterDefinition, AppearanceProfile,
        AppearanceProfileCatalog, AppearanceProfileId, BodyVariantDefinition, BodyVariantId,
        HUMAN_MORPH_TARGET_NAMES, MorphMappingSide, MorphTargetMapping, SpeciesId, UnitAppearance,
        UnitRenderKey,
    };

    use super::sync::sync_unit_appearance_morphs;

    let profile = AppearanceProfile {
        id: AppearanceProfileId::new("human"),
        species_id: SpeciesId::new("human"),
        schema_version: 1,
        height_scale_min: 0.85,
        height_scale_max: 1.15,
        height_scale_default: 1.0,
        body_variants: vec![BodyVariantDefinition {
            id: BodyVariantId::new("human_male"),
            display_name: "Male".into(),
            render_key: UnitRenderKey::reserved("human_male"),
            enabled: true,
        }],
        parameters: vec![AppearanceParameterDefinition {
            id: AppearanceParamId::new("build"),
            display_name: "Build".into(),
            category: "Body".into(),
            min: 0.0,
            max: 1.0,
            default: 0.5,
            display_order: 1,
            enabled: true,
        }],
        morph_mappings: vec![
            MorphTargetMapping {
                variant_id: BodyVariantId::new("human_male"),
                param_id: AppearanceParamId::new("build"),
                technical_target: "build_broad".into(),
                side: MorphMappingSide::AboveDefault,
                multiplier: 1.0,
                enabled: true,
            },
            MorphTargetMapping {
                variant_id: BodyVariantId::new("human_male"),
                param_id: AppearanceParamId::new("build"),
                technical_target: "build_narrow".into(),
                side: MorphMappingSide::BelowDefault,
                multiplier: 1.0,
                enabled: true,
            },
        ],
        enabled: true,
    };
    let catalog =
        AppearanceProfileCatalog::from_definitions(vec![profile]).expect("appearance catalog");

    let mut morphs = BTreeMap::new();
    morphs.insert(AppearanceParamId::new("build"), 0.0);
    let appearance = UnitAppearance {
        profile_id: AppearanceProfileId::new("human"),
        body_variant_id: BodyVariantId::new("human_male"),
        height_scale: 1.0,
        morphs,
        generation_seed: None,
    };

    let mut world = World::new();
    world.init_resource::<Assets<Mesh>>();
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0, 0.0, 0.0]]);
    mesh.set_morph_target_names(
        HUMAN_MORPH_TARGET_NAMES
            .iter()
            .map(|name| name.to_string())
            .collect(),
    );
    let mesh_handle = world.resource_mut::<Assets<Mesh>>().add(mesh);

    let root = world
        .spawn((
            UnitSceneRoot,
            UnitPresentationAppearance { appearance },
        ))
        .id();
    let primitive = world
        .spawn((
            Mesh3d(mesh_handle),
            MeshMorphWeights::new(vec![0.0; HUMAN_MORPH_TARGET_NAMES.len()]).unwrap(),
            ChildOf(root),
        ))
        .id();

    world.insert_resource(catalog);
    world
        .run_system_once(sync_unit_appearance_morphs)
        .expect("morph sync");

    let weights = world
        .entity(primitive)
        .get::<MeshMorphWeights>()
        .expect("primitive weights")
        .weights();
    assert!(weights[1] > 0.9, "build_narrow should be active at build min");
    assert!(weights[0] < 0.1, "build_broad should be inactive at build min");
}

#[test]
fn human_glb_has_multiple_independent_morph_weight_owners() {
    use std::path::PathBuf;

    let path = PathBuf::from("assets/units/human_male.glb");
    let (document, _, _) = gltf::import(&path).expect("import");
    let morph_mesh_count = document
        .meshes()
        .filter(|mesh| {
            mesh.primitives().next().is_some_and(|primitive| {
                !primitive.morph_targets().collect::<Vec<_>>().is_empty()
            })
        })
        .count();
    assert_eq!(
        morph_mesh_count,
        3,
        "human male should expose independent morph owners for face + body meshes"
    );
}
