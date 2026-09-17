//! CG5 equipment morph sync runtime tests.

use std::collections::BTreeMap;

use bevy::ecs::system::RunSystemOnce;
use bevy::mesh::morph::{MeshMorphWeights, MorphWeights};
use bevy::prelude::*;

use crate::units::components::UnitSceneRoot;
use crate::units::equipment_presentation::{
    UnitEquipmentMorphConfig, UnitEquipmentMorphFingerprint, UnitEquipmentSceneRoot,
    UnitEquipmentVisual, sync_unit_equipment_morphs,
};
use crate::units::presentation::UnitPresentationAppearance;
use crate::world::equipment::EquipmentSlot;
use crate::world::{
    AppearanceParamId, AppearanceParameterDefinition, AppearanceProfile,
    AppearanceProfileCatalog, AppearanceProfileId, BodyVariantDefinition, BodyVariantId,
    ItemInstanceId, MorphMappingSide, MorphTargetMapping, SpeciesId,
    UnitAppearance, UnitId, UnitRenderKey,
};

fn param(id: &str, default: f32) -> AppearanceParameterDefinition {
    AppearanceParameterDefinition {
        id: AppearanceParamId::new(id),
        display_name: id.into(),
        category: "Body".into(),
        min: 0.0,
        max: 1.0,
        default,
        display_order: 1,
        enabled: true,
    }
}

fn mapping(
    variant: &str,
    param: &str,
    target: &str,
    side: MorphMappingSide,
) -> MorphTargetMapping {
    MorphTargetMapping {
        variant_id: BodyVariantId::new(variant),
        param_id: AppearanceParamId::new(param),
        technical_target: target.into(),
        side,
        multiplier: 1.0,
        enabled: true,
    }
}

fn human_profile() -> AppearanceProfile {
    AppearanceProfile {
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
        parameters: vec![
            param("build", 0.5),
            param("fat", 0.35),
            param("muscle", 0.45),
            param("head_size", 0.5),
        ],
        morph_mappings: vec![
            mapping("human_male", "build", "build_broad", MorphMappingSide::AboveDefault),
            mapping("human_male", "build", "build_narrow", MorphMappingSide::BelowDefault),
            mapping("human_male", "fat", "fat_soft", MorphMappingSide::AboveDefault),
            mapping("human_male", "muscle", "muscle_define", MorphMappingSide::AboveDefault),
            mapping("human_male", "head_size", "head_large", MorphMappingSide::AboveDefault),
            mapping("human_male", "head_size", "head_small", MorphMappingSide::BelowDefault),
        ],
        enabled: true,
    }
}

fn spawn_test_mesh(world: &mut World, target_names: &[&str]) -> Handle<Mesh> {
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0, 0.0, 0.0]]);
    mesh.set_morph_target_names(target_names.iter().map(|name| name.to_string()).collect());
    world.resource_mut::<Assets<Mesh>>().add(mesh)
}

fn spawn_armored_unit(
    world: &mut World,
    appearance: UnitAppearance,
    consumed: Vec<AppearanceParamId>,
    mesh_handle: Handle<Mesh>,
) -> (Entity, Entity) {
    let unit = world
        .spawn((
            UnitSceneRoot,
            UnitPresentationAppearance { appearance },
        ))
        .id();
    let equipment = world
        .spawn((
            UnitEquipmentVisual {
                unit_id: UnitId::new(1),
                slot: EquipmentSlot::Body,
                item_instance_id: ItemInstanceId::new(1),
            },
            UnitEquipmentMorphConfig {
                consumed_morph_params: consumed,
            },
            UnitEquipmentSceneRoot,
            ChildOf(unit),
        ))
        .id();
    let primitive = world
        .spawn((
            Mesh3d(mesh_handle),
            MeshMorphWeights::new(vec![0.0; 4]).unwrap(),
            ChildOf(equipment),
        ))
        .id();
    let _ = primitive;
    (unit, equipment)
}

#[test]
fn equipment_morph_sync_writes_build_broad_for_max_build() {
    let mut world = World::new();
    world.init_resource::<Assets<Mesh>>();
    let mesh = spawn_test_mesh(
        &mut world,
        &["build_broad", "build_narrow", "fat_soft", "muscle_define"],
    );
    let mut morphs = BTreeMap::new();
    morphs.insert(AppearanceParamId::new("build"), 1.0);
    let appearance = UnitAppearance {
        profile_id: AppearanceProfileId::new("human"),
        body_variant_id: BodyVariantId::new("human_male"),
        height_scale: 1.0,
        morphs,
        generation_seed: None,
    };
    let (_, equipment) = spawn_armored_unit(
        &mut world,
        appearance,
        vec![
            AppearanceParamId::new("build"),
            AppearanceParamId::new("fat"),
            AppearanceParamId::new("muscle"),
        ],
        mesh,
    );
    world.insert_resource(AppearanceProfileCatalog::from_definitions(vec![human_profile()]).unwrap());
    world.run_system_once(sync_unit_equipment_morphs).expect("sync");

    let primitive = world
        .query_filtered::<Entity, With<MeshMorphWeights>>()
        .iter(&world)
        .next()
        .expect("primitive");
    let weights = world.entity(primitive).get::<MeshMorphWeights>().unwrap().weights();
    assert!(weights[0] > 0.9);
    assert!(weights[1] < 0.1);
    assert!(world.entity(equipment).contains::<UnitEquipmentMorphFingerprint>());
}

#[test]
fn equipment_morph_sync_ignores_unconsumed_head_size() {
    let mut world = World::new();
    world.init_resource::<Assets<Mesh>>();
    let mesh = spawn_test_mesh(
        &mut world,
        &["build_broad", "build_narrow", "fat_soft", "muscle_define"],
    );
    let mut morphs = BTreeMap::new();
    morphs.insert(AppearanceParamId::new("head_size"), 1.0);
    let appearance = UnitAppearance {
        profile_id: AppearanceProfileId::new("human"),
        body_variant_id: BodyVariantId::new("human_male"),
        height_scale: 1.0,
        morphs,
        generation_seed: None,
    };
    spawn_armored_unit(
        &mut world,
        appearance,
        vec![
            AppearanceParamId::new("build"),
            AppearanceParamId::new("fat"),
            AppearanceParamId::new("muscle"),
        ],
        mesh,
    );
    world.insert_resource(AppearanceProfileCatalog::from_definitions(vec![human_profile()]).unwrap());
    world.run_system_once(sync_unit_equipment_morphs).expect("sync");

    let primitive = world
        .query_filtered::<Entity, With<MeshMorphWeights>>()
        .iter(&world)
        .next()
        .expect("primitive");
    let weights = world.entity(primitive).get::<MeshMorphWeights>().unwrap().weights();
    assert!(weights.iter().all(|weight| *weight < 1e-4));
}

#[test]
fn equipment_morph_sync_reapplies_on_appearance_change() {
    let mut world = World::new();
    world.init_resource::<Assets<Mesh>>();
    let mesh = spawn_test_mesh(
        &mut world,
        &["build_broad", "build_narrow", "fat_soft", "muscle_define"],
    );
    let mut morphs = BTreeMap::new();
    morphs.insert(AppearanceParamId::new("build"), 1.0);
    let appearance = UnitAppearance {
        profile_id: AppearanceProfileId::new("human"),
        body_variant_id: BodyVariantId::new("human_male"),
        height_scale: 1.0,
        morphs,
        generation_seed: None,
    };
    let (unit, _) = spawn_armored_unit(
        &mut world,
        appearance,
        vec![AppearanceParamId::new("build")],
        mesh,
    );
    world.insert_resource(AppearanceProfileCatalog::from_definitions(vec![human_profile()]).unwrap());
    world.run_system_once(sync_unit_equipment_morphs).expect("sync");

    let mut morphs = BTreeMap::new();
    morphs.insert(AppearanceParamId::new("build"), 0.0);
    world.entity_mut(unit).insert(UnitPresentationAppearance {
        appearance: UnitAppearance {
            profile_id: AppearanceProfileId::new("human"),
            body_variant_id: BodyVariantId::new("human_male"),
            height_scale: 1.0,
            morphs,
            generation_seed: None,
        },
    });
    world.run_system_once(sync_unit_equipment_morphs).expect("sync");

    let primitive = world
        .query_filtered::<Entity, With<MeshMorphWeights>>()
        .iter(&world)
        .next()
        .expect("primitive");
    let weights = world.entity(primitive).get::<MeshMorphWeights>().unwrap().weights();
    assert!(weights[1] > 0.9);
}

#[test]
fn two_equipment_instances_share_mesh_but_keep_independent_weights() {
    let mut world = World::new();
    world.init_resource::<Assets<Mesh>>();
    let mesh = spawn_test_mesh(
        &mut world,
        &["build_broad", "build_narrow", "fat_soft", "muscle_define"],
    );

    let mut broad = BTreeMap::new();
    broad.insert(AppearanceParamId::new("build"), 1.0);
    let mut narrow = BTreeMap::new();
    narrow.insert(AppearanceParamId::new("build"), 0.0);

    let unit_a = world
        .spawn((
            UnitSceneRoot,
            UnitPresentationAppearance {
                appearance: UnitAppearance {
                    profile_id: AppearanceProfileId::new("human"),
                    body_variant_id: BodyVariantId::new("human_male"),
                    height_scale: 1.0,
                    morphs: broad,
                    generation_seed: None,
                },
            },
        ))
        .id();
    let unit_b = world
        .spawn((
            UnitSceneRoot,
            UnitPresentationAppearance {
                appearance: UnitAppearance {
                    profile_id: AppearanceProfileId::new("human"),
                    body_variant_id: BodyVariantId::new("human_male"),
                    height_scale: 1.0,
                    morphs: narrow,
                    generation_seed: None,
                },
            },
        ))
        .id();

    for (unit, unit_id) in [(unit_a, 1), (unit_b, 2)] {
        let equipment = world
            .spawn((
                UnitEquipmentVisual {
                    unit_id: UnitId::new(unit_id),
                    slot: EquipmentSlot::Body,
                    item_instance_id: ItemInstanceId::new(unit_id as u32),
                },
                UnitEquipmentMorphConfig {
                    consumed_morph_params: vec![AppearanceParamId::new("build")],
                },
                UnitEquipmentSceneRoot,
                ChildOf(unit),
            ))
            .id();
        world
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMorphWeights::new(vec![0.0; 4]).unwrap(),
                ChildOf(equipment),
            ));
    }

    world.insert_resource(AppearanceProfileCatalog::from_definitions(vec![human_profile()]).unwrap());
    world.run_system_once(sync_unit_equipment_morphs).expect("sync");

    let mut seen_broad = false;
    let mut seen_narrow = false;
    for entity in world
        .query_filtered::<Entity, With<MeshMorphWeights>>()
        .iter(&world)
    {
        let weights = world.entity(entity).get::<MeshMorphWeights>().unwrap().weights();
        if weights[0] > 0.9 {
            seen_broad = true;
        }
        if weights[1] > 0.9 {
            seen_narrow = true;
        }
    }
    assert!(seen_broad && seen_narrow);
}

#[test]
fn equipment_morph_sync_removes_parent_morph_weights() {
    let mut world = World::new();
    world.init_resource::<Assets<Mesh>>();
    let mesh = spawn_test_mesh(
        &mut world,
        &["build_broad", "build_narrow", "fat_soft", "muscle_define"],
    );
    let mut morphs = BTreeMap::new();
    morphs.insert(AppearanceParamId::new("build"), 1.0);
    let unit = world
        .spawn((
            UnitSceneRoot,
            UnitPresentationAppearance {
                appearance: UnitAppearance {
                    profile_id: AppearanceProfileId::new("human"),
                    body_variant_id: BodyVariantId::new("human_male"),
                    height_scale: 1.0,
                    morphs,
                    generation_seed: None,
                },
            },
        ))
        .id();
    let equipment = world
        .spawn((
            UnitEquipmentVisual {
                unit_id: UnitId::new(1),
                slot: EquipmentSlot::Body,
                item_instance_id: ItemInstanceId::new(1),
            },
            UnitEquipmentMorphConfig {
                consumed_morph_params: vec![AppearanceParamId::new("build")],
            },
            UnitEquipmentSceneRoot,
            ChildOf(unit),
        ))
        .id();
    let parent = world
        .spawn((
            MorphWeights::new(vec![0.0; 4], None).unwrap(),
            ChildOf(equipment),
        ))
        .id();
    world.spawn((
        Mesh3d(mesh),
        MeshMorphWeights::new(vec![0.0; 4]).unwrap(),
        ChildOf(parent),
    ));
    world.insert_resource(AppearanceProfileCatalog::from_definitions(vec![human_profile()]).unwrap());
    world.run_system_once(sync_unit_equipment_morphs).expect("sync");
    assert!(world.entity(parent).get::<MorphWeights>().is_none());
}
