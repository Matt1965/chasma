//! Offline verification of installed human player GLB assets.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::data_import::asset_sizing::{
    measure_glb_source_bounds, unit_default_desired_height_meters,
};

const EXPECTED_CLIPS: &[&str] = &[
    "Mine",
    "Idle",
    "Walk",
    "Run",
    "Death",
    "Hit",
    "Punch_Jab",
    "Punch_Cross",
    "Sword_Attack",
    "Sword_Idle",
];

fn human_glb_path(render_key: &str) -> PathBuf {
    PathBuf::from("assets")
        .join("units")
        .join(format!("{render_key}.glb"))
}

fn animation_duration_seconds(animation: &gltf::Animation, buffers: &[gltf::buffer::Data]) -> f32 {
    let mut max_t = 0.0f32;
    for channel in animation.channels() {
        let reader = channel.reader(|buffer| Some(&buffers[buffer.index()].0));
        if let Some(inputs) = reader.read_inputs() {
            for t in inputs {
                max_t = max_t.max(t);
            }
        }
    }
    max_t
}

fn glb_animation_inventory(path: &std::path::Path) -> Vec<(usize, String, f32, usize)> {
    let (document, buffers, _) = gltf::import(path).expect("glb import");
    document
        .animations()
        .enumerate()
        .map(|(index, animation)| {
            let name = animation.name().unwrap_or("").to_string();
            let duration = animation_duration_seconds(&animation, &buffers);
            let nodes: HashSet<usize> = animation
                .channels()
                .map(|channel| channel.target().node().index())
                .collect();
            (index, name, duration, nodes.len())
        })
        .collect()
}

fn assert_human_glb(render_key: &str) {
    let path = human_glb_path(render_key);
    assert!(path.is_file(), "missing runtime asset {}", path.display());

    let (bounds, _, _) = measure_glb_source_bounds(&path, None, None).expect("bounds");
    let height = bounds.height_meters;
    assert!(
        height > 1.5 && height < 2.1,
        "{render_key} mesh height {height}m outside humanoid range",
    );

    let desired = unit_default_desired_height_meters(render_key, Some(render_key), 0.4);
    assert!(
        (desired - 1.75).abs() < f32::EPSILON,
        "expected 1.75m sizing target for {render_key}",
    );

    let clips = glb_animation_inventory(&path);
    assert_eq!(
        clips.len(),
        EXPECTED_CLIPS.len(),
        "{render_key} clip count (got {:?})",
        clips.iter().map(|(_, n, _, _)| n.as_str()).collect::<Vec<_>>()
    );
    let names: std::collections::HashSet<&str> =
        clips.iter().map(|(_, name, _, _)| name.as_str()).collect();
    for expected_name in EXPECTED_CLIPS {
        assert!(names.contains(expected_name), "missing clip {expected_name}");
    }
    for (_, name, duration, node_count) in &clips {
        assert!(*duration > 0.0, "{name} duration");
        assert!(*node_count > 0, "{name} should affect skeleton nodes");
    }
}

#[test]
fn human_male_glb_has_expected_clips_and_scale() {
    assert_human_glb("human_male");
}

#[test]
fn human_female_glb_has_expected_clips_and_scale() {
    assert_human_glb("human_female");
}

fn glb_node_names(render_key: &str) -> std::collections::HashSet<String> {
    let path = human_glb_path(render_key);
    let (document, _, _) = gltf::import(&path).expect("glb import");
    document
        .nodes()
        .filter_map(|node| node.name().map(str::to_string))
        .collect()
}

fn has_node_suffix(names: &std::collections::HashSet<String>, suffix: &str) -> bool {
    names
        .iter()
        .any(|name| name.ends_with(suffix) || name == suffix)
}

const ARM_CHAIN_SUFFIXES: &[&str] = &[
    "clavicle_l",
    "clavicle_r",
    "upperarm_l",
    "upperarm_r",
    "lowerarm_l",
    "lowerarm_r",
    "hand_l",
    "hand_r",
];

const LOCOMOTION_CLIP_NAMES: &[&str] = &["Idle", "Walk", "Run"];

fn glb_node_name_index(path: &std::path::Path) -> std::collections::HashMap<String, usize> {
    let (document, _, _) = gltf::import(path).expect("glb import");
    document
        .nodes()
        .filter_map(|node| node.name().map(|name| (name.to_string(), node.index())))
        .collect()
}

fn clip_channels(
    path: &std::path::Path,
    clip_name: &str,
) -> Vec<(usize, gltf::animation::Property)> {
    let (document, _, _) = gltf::import(path).expect("glb import");
    let animation = document
        .animations()
        .find(|animation| animation.name() == Some(clip_name))
        .expect("clip");
    animation
        .channels()
        .map(|channel| (channel.target().node().index(), channel.target().property()))
        .collect()
}

fn clip_has_scale_channels(path: &std::path::Path, clip_name: &str) -> bool {
    clip_channels(path, clip_name)
        .iter()
        .any(|(_, property)| *property == gltf::animation::Property::Scale)
}

fn clip_animates_arm_chain(path: &std::path::Path, clip_name: &str) -> bool {
    let node_index = glb_node_name_index(path);
    let channels = clip_channels(path, clip_name);
    ARM_CHAIN_SUFFIXES.iter().all(|suffix| {
        let Some(node) = node_index
            .iter()
            .find(|(name, _)| name.ends_with(*suffix) || name.as_str() == *suffix)
            .map(|(_, index)| *index)
        else {
            return false;
        };
        channels.iter().any(|(indexed, property)| {
            *indexed == node
                && (*property == gltf::animation::Property::Rotation
                    || *property == gltf::animation::Property::Translation)
        })
    })
}

#[test]
fn human_glb_has_equipment_attachment_bones() {
    for render_key in ["human_male", "human_female"] {
        let names = glb_node_names(render_key);
        assert!(
            has_node_suffix(&names, "hand_r"),
            "{render_key} missing hand_r weapon-socket bone node"
        );
        assert!(
            has_node_suffix(&names, "hand_l"),
            "{render_key} missing hand_l offhand-socket bone node"
        );
        assert!(
            has_node_suffix(&names, "Head"),
            "{render_key} missing Head bone node"
        );
        assert!(
            has_node_suffix(&names, "spine_02"),
            "{render_key} missing spine_02 back-attach bone node"
        );
    }
}

fn gltf_skin_joint_names(path: &std::path::Path) -> Vec<String> {
    let (document, _, _) = gltf::import(path).expect("glb import");
    document
        .skins()
        .next()
        .map(|skin| {
            skin.joints()
                .map(|node| node.name().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn equipment_armor_shares_human_skeleton_joint_names() {
    for (unit_key, armor_path) in [
        ("human_male", "items/equipment/human_male/ranger_body.glb"),
        (
            "human_female",
            "items/equipment/human_female/ranger_body.glb",
        ),
    ] {
        let human = gltf_skin_joint_names(&human_glb_path(unit_key));
        let armor_full = std::path::PathBuf::from("assets").join(armor_path);
        let armor = gltf_skin_joint_names(&armor_full);
        assert_eq!(human.len(), 65, "{unit_key} human joint count");
        assert_eq!(armor.len(), 65, "{unit_key} armor joint count");
        assert_eq!(
            human, armor,
            "{unit_key} armor joints must match human skeleton"
        );
    }
}

#[test]
fn locomotion_clips_animate_arm_chains() {
    for render_key in ["human_male", "human_female"] {
        let path = human_glb_path(render_key);
        for clip_name in LOCOMOTION_CLIP_NAMES {
            assert!(
                clip_animates_arm_chain(&path, clip_name),
                "{render_key} {clip_name} missing arm-chain animation targets",
            );
        }
    }
}

#[test]
fn retargeted_clips_have_no_animated_scale_channels() {
    for render_key in ["human_male", "human_female"] {
        let path = human_glb_path(render_key);
        for clip_name in EXPECTED_CLIPS {
            assert!(
                !clip_has_scale_channels(&path, clip_name),
                "{render_key} {clip_name} should not contain animated scale tracks",
            );
        }
    }
}

#[test]
fn human_glbs_expose_cg2_morph_targets() {
    use crate::world::HUMAN_MORPH_TARGET_NAMES;

    for render_key in ["human_male", "human_female"] {
        let path = human_glb_path(render_key);
        let (document, _, _) = gltf::import(&path).expect("glb import");
        let body_mesh = document
            .meshes()
            .find(|mesh| {
                mesh.name()
                    .is_some_and(|name| !name.contains("Eye") && !name.contains("brow"))
            })
            .expect("body mesh");
        let target_count = body_mesh
            .primitives()
            .next()
            .expect("primitive")
            .morph_targets()
            .count();
        assert_eq!(
            target_count,
            HUMAN_MORPH_TARGET_NAMES.len(),
            "{render_key} morph target count"
        );
        assert!(
            target_count <= 16,
            "morph target count must stay within Bevy 0.18 per-mesh limit",
        );
    }
}

#[test]
fn human_male_and_female_glb_assets_are_distinct_bodies() {
    let male_path = human_glb_path("human_male");
    let female_path = human_glb_path("human_female");
    let male_bytes = std::fs::read(&male_path).expect("read male glb");
    let female_bytes = std::fs::read(&female_path).expect("read female glb");
    assert_ne!(
        male_bytes,
        female_bytes,
        "human_female.glb must not be a byte-identical copy of human_male.glb"
    );

    let male_meshes: HashSet<String> = {
        let (document, _, _) = gltf::import(&male_path).expect("male import");
        document
            .meshes()
            .filter_map(|mesh| mesh.name().map(str::to_string))
            .collect()
    };
    let female_meshes: HashSet<String> = {
        let (document, _, _) = gltf::import(&female_path).expect("female import");
        document
            .meshes()
            .filter_map(|mesh| mesh.name().map(str::to_string))
            .collect()
    };
    assert_ne!(male_meshes, female_meshes);
    assert!(
        female_meshes.iter().any(|name| name.contains("Female") || name.contains("female")),
        "female GLB should contain a female body mesh (got {:?})",
        female_meshes
    );
}

struct MorphTargetDeltaStats {
    non_zero_vertices: usize,
    vertex_count: usize,
    max_displacement: f32,
    rms_displacement: f32,
}

fn morph_target_delta_stats(
    primitive: &gltf::Primitive,
    target_index: usize,
    buffers: &[gltf::buffer::Data],
) -> MorphTargetDeltaStats {
    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()].0));
    let positions = reader
        .read_positions()
        .expect("position data")
        .collect::<Vec<_>>();
    let vertex_count = positions.len();
    let (position_deltas, _, _) = reader
        .read_morph_targets()
        .nth(target_index)
        .expect("morph target");
    let deltas = position_deltas
        .expect("morph position deltas")
        .collect::<Vec<_>>();
    assert_eq!(deltas.len(), vertex_count);
    let mut non_zero_vertices = 0usize;
    let mut max_displacement = 0.0f32;
    let mut sum_sq = 0.0f32;
    for delta in deltas {
        let magnitude = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        if magnitude > 1e-6 {
            non_zero_vertices += 1;
        }
        max_displacement = max_displacement.max(magnitude);
        sum_sq += magnitude * magnitude;
    }
    MorphTargetDeltaStats {
        non_zero_vertices,
        vertex_count,
        max_displacement,
        rms_displacement: (sum_sq / vertex_count.max(1) as f32).sqrt(),
    }
}

fn primary_body_mesh<'a>(
    document: &'a gltf::Document,
    buffers: &'a [gltf::buffer::Data],
) -> gltf::Mesh<'a> {
    document
        .meshes()
        .filter(|mesh| {
            mesh.name().is_some_and(|name| {
                let lower = name.to_ascii_lowercase();
                !lower.contains("eye") && !lower.contains("brow") && !lower.contains("face")
            })
        })
        .max_by_key(|mesh| {
            mesh.primitives()
                .next()
                .map(|primitive| {
                    primitive
                        .reader(|buffer| Some(&buffers[buffer.index()].0))
                        .read_positions()
                        .map(|iter| iter.count())
                        .unwrap_or(0)
                })
                .unwrap_or(0)
        })
        .expect("body mesh")
}

#[test]
fn human_glbs_have_meaningful_body_morph_deltas() {
    use crate::world::HUMAN_MORPH_TARGET_NAMES;

    for render_key in ["human_male", "human_female"] {
        let path = human_glb_path(render_key);
        let (document, buffers, _) = gltf::import(&path).expect("import");
        assert!(
            document
                .animations()
                .flat_map(|animation| animation.channels())
                .all(|channel| {
                    channel.target().property() != gltf::animation::Property::MorphTargetWeights
                }),
            "{render_key} animations must not target morph weights",
        );
        let mesh = primary_body_mesh(&document, &buffers);
        let primitive = mesh.primitives().next().expect("primitive");
        let target_count = primitive.morph_targets().count();
        assert_eq!(target_count, HUMAN_MORPH_TARGET_NAMES.len());
        for (index, name) in HUMAN_MORPH_TARGET_NAMES.iter().enumerate() {
            let stats = morph_target_delta_stats(&primitive, index, &buffers);
            let pct = stats.non_zero_vertices as f32 * 100.0 / stats.vertex_count as f32;
            assert!(
                stats.non_zero_vertices > 0,
                "{render_key} {name} has no non-zero POSITION deltas",
            );
            assert!(
                stats.max_displacement.is_finite() && stats.max_displacement > 0.0,
                "{render_key} {name} max displacement must be positive",
            );
            match *name {
                "build_broad" | "build_narrow" | "fat_soft" | "muscle_define" => {
                    assert!(pct >= 15.0, "{render_key} {name} affects too few body vertices");
                    assert!(
                        stats.max_displacement >= 0.015,
                        "{render_key} {name} displacement too small for visible body change",
                    );
                }
                "head_large" | "head_small" => {
                    assert!(pct >= 15.0, "{render_key} {name} affects too few head vertices");
                    assert!(
                        stats.max_displacement >= 0.01,
                        "{render_key} {name} displacement too small for visible head change",
                    );
                }
                _ => {}
            }
        }
    }
}

#[test]
fn head_large_materially_changes_body_bounds() {
    for render_key in ["human_male", "human_female"] {
        let path = human_glb_path(render_key);
        let (document, buffers, _) = gltf::import(&path).expect("import");
        let mesh = primary_body_mesh(&document, &buffers);
        let primitive = mesh.primitives().next().expect("primitive");
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()].0));
        let positions = reader
            .read_positions()
            .expect("positions")
            .collect::<Vec<_>>();
        let (head_large_deltas, _, _) = reader
            .read_morph_targets()
            .nth(4)
            .expect("head_large");
        let head_large = head_large_deltas
            .expect("head_large deltas")
            .collect::<Vec<_>>();
        let mut base_min = [f32::INFINITY; 3];
        let mut base_max = [f32::NEG_INFINITY; 3];
        let mut morphed_min = [f32::INFINITY; 3];
        let mut morphed_max = [f32::NEG_INFINITY; 3];
        for (base, delta) in positions.iter().zip(head_large.iter()) {
            let morphed = [
                base[0] + delta[0],
                base[1] + delta[1],
                base[2] + delta[2],
            ];
            for axis in 0..3 {
                base_min[axis] = base_min[axis].min(base[axis]);
                base_max[axis] = base_max[axis].max(base[axis]);
                morphed_min[axis] = morphed_min[axis].min(morphed[axis]);
                morphed_max[axis] = morphed_max[axis].max(morphed[axis]);
            }
        }
        let base_height = base_max[1] - base_min[1];
        let morphed_height = morphed_max[1] - morphed_min[1];
        assert!(
            morphed_height > base_height + 0.005,
            "{render_key} head_large should increase body bounds height (base={base_height:.4}, morphed={morphed_height:.4})",
        );
    }
}

#[test]
fn human_glbs_expose_bevy_mesh_extras_target_names() {
    use crate::world::HUMAN_MORPH_TARGET_NAMES;

    for render_key in ["human_male", "human_female"] {
        let path = human_glb_path(render_key);
        let (document, _, _) = gltf::import(&path).expect("import");
        for mesh in document.meshes() {
            let primitive = mesh.primitives().next().expect("primitive");
            if primitive.morph_targets().count() == 0 {
                continue;
            }
            let extras = mesh
                .extras()
                .as_ref()
                .map(|value| value.get())
                .filter(|value| !value.is_empty())
                .expect("morph mesh extras");
            assert!(
                extras.contains("targetNames"),
                "{render_key} mesh {:?} extras missing targetNames key: {extras}",
                mesh.name(),
            );
            for name in HUMAN_MORPH_TARGET_NAMES {
                assert!(
                    extras.contains(name),
                    "{render_key} mesh {:?} extras missing {name}: {extras}",
                    mesh.name(),
                );
            }
        }
    }
}

#[test]
fn human_morph_owners_share_identical_target_count() {
    use crate::world::HUMAN_MORPH_TARGET_NAMES;

    for render_key in ["human_male", "human_female"] {
        let path = human_glb_path(render_key);
        let (document, _, _) = gltf::import(&path).expect("import");
        let counts = document
            .meshes()
            .filter_map(|mesh| {
                mesh.primitives()
                    .next()
                    .map(|primitive| primitive.morph_targets().count())
                    .filter(|count| *count > 0)
            })
            .collect::<Vec<_>>();
        assert_eq!(counts.len(), 3, "{render_key} should have three morph owners");
        assert!(
            counts
                .windows(2)
                .all(|pair| pair[0] == pair[1] && pair[0] == HUMAN_MORPH_TARGET_NAMES.len()),
            "{render_key} morph owners must share the CG2 target count",
        );
    }
}

const CG5_TORSO_TARGETS: &[&str] = &[
    "build_broad",
    "build_narrow",
    "fat_soft",
    "muscle_define",
];
const CG5_HEAD_TARGETS: &[&str] = &["head_large", "head_small"];

fn equipment_glb_path(unit_key: &str, asset_name: &str) -> PathBuf {
    PathBuf::from("assets")
        .join("items")
        .join("equipment")
        .join(unit_key)
        .join(format!("{asset_name}.glb"))
}

fn equipment_mesh_extras_target_names(path: &std::path::Path) -> Vec<String> {
    let (document, _, _) = gltf::import(path).expect("import");
    let mesh = document
        .meshes()
        .find(|mesh| {
            mesh.primitives().next().is_some_and(|primitive| {
                !primitive.morph_targets().collect::<Vec<_>>().is_empty()
            })
        })
        .expect("morph mesh");
    let extras = mesh
        .extras()
        .as_ref()
        .map(|value| value.get())
        .filter(|value| !value.is_empty())
        .expect("mesh extras");
    assert!(
        extras.contains("targetNames"),
        "mesh extras missing targetNames: {extras}",
    );
    extras
        .split("\"targetNames\"")
        .nth(1)
        .and_then(|tail| tail.split('[').nth(1))
        .and_then(|body| body.split(']').next())
        .expect("targetNames array")
        .split(',')
        .filter_map(|part| {
            let trimmed = part.trim().trim_matches('"');
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect()
}

#[test]
fn cg5_equipment_glbs_expose_authored_morph_targets() {
    let cases = [
        ("human_male", "ranger_body", CG5_TORSO_TARGETS),
        ("human_male", "ranger_arms", CG5_TORSO_TARGETS),
        ("human_male", "ranger_legs", CG5_TORSO_TARGETS),
        ("human_male", "ranger_hood", CG5_HEAD_TARGETS),
        ("human_female", "ranger_body", CG5_TORSO_TARGETS),
        ("human_female", "ranger_hood", CG5_HEAD_TARGETS),
    ];
    for (unit_key, asset_name, expected_targets) in cases {
        let path = equipment_glb_path(unit_key, asset_name);
        assert!(path.is_file(), "missing {}", path.display());
        let (document, buffers, _) = gltf::import(&path).expect("import");
        let mesh = document
            .meshes()
            .find(|mesh| {
                mesh.primitives().next().is_some_and(|primitive| {
                    primitive.morph_targets().count() > 0
                })
            })
            .expect("morph mesh");
        let primitive = mesh.primitives().next().expect("primitive");
        assert_eq!(primitive.morph_targets().count(), expected_targets.len());
        assert_eq!(equipment_mesh_extras_target_names(&path), expected_targets);
        for (index, name) in expected_targets.iter().enumerate() {
            let stats = morph_target_delta_stats(&primitive, index, &buffers);
            assert!(
                stats.non_zero_vertices > 0,
                "{unit_key}/{asset_name} {name} has no non-zero deltas",
            );
            assert!(
                stats.max_displacement.is_finite() && stats.max_displacement > 1e-5,
                "{unit_key}/{asset_name} {name} displacement too small",
            );
        }
        assert!(
            expected_targets.len() <= 16,
            "morph target count must stay within Bevy per-mesh limit",
        );
    }
}

#[test]
fn cg5_ranger_body_bevy_loaded_mesh_exposes_morph_target_names() {
    use bevy::asset::LoadState;
    use bevy::gltf::{Gltf, GltfMesh};
    use bevy::prelude::*;

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
        .load("items/equipment/human_male/ranger_body.glb");
    let gltf_id = gltf_handle.id();
    let mut loaded = false;
    for _ in 0..500 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(5));
        if app.world().resource::<AssetServer>().is_loaded_with_dependencies(gltf_id)
            && app.world().resource::<Assets<Gltf>>().get(&gltf_handle).is_some()
        {
            loaded = true;
            break;
        }
        if matches!(
            app.world().resource::<AssetServer>().get_load_state(gltf_id),
            Some(LoadState::Failed(_))
        ) {
            panic!("ranger_body.glb load failed");
        }
    }
    assert!(loaded, "timed out loading ranger_body.glb");

    let gltf = app.world().resource::<Assets<Gltf>>().get(&gltf_handle).unwrap();
    let gltf_meshes = app.world().resource::<Assets<GltfMesh>>();
    let meshes = app.world().resource::<Assets<Mesh>>();
    let mut armor_mesh: Option<&Mesh> = None;
    for gltf_mesh_handle in &gltf.meshes {
        let gltf_mesh = gltf_meshes.get(gltf_mesh_handle).expect("gltf mesh");
        for primitive in &gltf_mesh.primitives {
            let mesh = meshes.get(&primitive.mesh).expect("bevy mesh");
            if mesh.morph_targets().is_some() {
                armor_mesh = Some(mesh);
                break;
            }
        }
    }
    let armor_mesh = armor_mesh.expect("armor morph mesh");
    let names = armor_mesh
        .morph_target_names()
        .expect("armor morph_target_names");
    assert_eq!(names, CG5_TORSO_TARGETS);
}

#[test]
fn human_male_and_female_share_compatible_animation_structure() {
    let male = glb_animation_inventory(&human_glb_path("human_male"));
    let female = glb_animation_inventory(&human_glb_path("human_female"));
    assert_eq!(male.len(), female.len());
    for ((_, male_name, male_dur, male_nodes), (_, female_name, female_dur, female_nodes)) in
        male.iter().zip(female.iter())
    {
        assert_eq!(male_name, female_name);
        if male_name == "Mine" {
            // Mine is preserved verbatim from each body's source export, not UAL-retargeted.
            continue;
        }
        assert!(
            (male_dur - female_dur).abs() < 0.05,
            "duration mismatch for {male_name}",
        );
        assert_eq!(
            male_nodes, female_nodes,
            "node count mismatch for {male_name}",
        );
    }
}
