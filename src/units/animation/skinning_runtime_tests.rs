//! Runtime skinning gate: load unit GLBs through Bevy's real glTF loader, spawn the scene,
//! optionally play a clip, and evaluate linear-blend skinning exactly as the GPU does
//! (`joint GlobalTransform * inverse bindpose`). Catches export defects that offline
//! glTF-spec checks miss because Bevy's loader/animator is the consumer that matters.

use std::path::PathBuf;

use bevy::asset::LoadState;
use bevy::gltf::Gltf;
use bevy::mesh::VertexAttributeValues;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;

/// Per skinned mesh: worst and median vertex deviation from the rest reference (meters).
#[derive(Debug, Clone)]
pub struct SkinDeviation {
    pub mesh: String,
    pub vertices: usize,
    pub max_m: f32,
    pub median_m: f32,
    pub extent_m: f32,
}

fn headless_app() -> App {
    let asset_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: asset_root.to_string_lossy().into_owned(),
            ..default()
        },
        bevy::transform::TransformPlugin,
        bevy::image::ImagePlugin::default(),
        bevy::mesh::MeshPlugin,
        bevy::scene::ScenePlugin,
        bevy::animation::AnimationPlugin,
        bevy::gltf::GltfPlugin::default(),
    ));
    app.init_asset::<bevy::pbr::StandardMaterial>();
    app.init_asset::<SkinnedMeshInverseBindposes>();
    app.register_type::<bevy::pbr::MeshMaterial3d<bevy::pbr::StandardMaterial>>();
    app.register_type::<bevy::mesh::morph::MorphWeights>();
    app.register_type::<bevy::mesh::morph::MeshMorphWeights>();
    app.finish();
    app.cleanup();
    app
}

fn wait_loaded(app: &mut App, handle: &Handle<Gltf>) {
    let id = handle.id();
    for _ in 0..2000 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let server = app.world().resource::<AssetServer>();
        if let Some(LoadState::Failed(err)) = server.get_load_state(id) {
            panic!("glb load failed: {err}");
        }
        if server.is_loaded_with_dependencies(id)
            && app.world().resource::<Assets<Gltf>>().get(handle).is_some()
        {
            return;
        }
    }
    panic!("timed out loading glb");
}

fn evaluate(app: &mut App) -> Vec<SkinDeviation> {
    let world = app.world_mut();
    let mut rows = Vec::new();
    let mut query = world.query::<(&Name, &Mesh3d, &SkinnedMesh, &GlobalTransform)>();
    let data: Vec<(String, Handle<Mesh>, SkinnedMesh, GlobalTransform)> = query
        .iter(world)
        .map(|(n, m, s, g)| (n.to_string(), m.0.clone(), s.clone(), *g))
        .collect();
    for (name, mesh_handle, skinned, mesh_global) in data {
        let meshes = world.resource::<Assets<Mesh>>();
        let ibps = world.resource::<Assets<SkinnedMeshInverseBindposes>>();
        let Some(mesh) = meshes.get(&mesh_handle) else { continue };
        let Some(ibp) = ibps.get(&skinned.inverse_bindposes) else { continue };
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        let Some(VertexAttributeValues::Uint16x4(joint_idx)) =
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX)
        else {
            continue;
        };
        let Some(VertexAttributeValues::Float32x4(weights)) =
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)
        else {
            continue;
        };
        let joint_mats: Vec<Mat4> = skinned
            .joints
            .iter()
            .zip(ibp.iter())
            .map(|(joint, inv)| {
                world
                    .get::<GlobalTransform>(*joint)
                    .map(|g| g.to_matrix() * *inv)
                    .unwrap_or(Mat4::IDENTITY)
            })
            .collect();
        let mesh_mat = mesh_global.to_matrix();
        let mut devs = Vec::with_capacity(positions.len());
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for ((p, ji), w) in positions.iter().zip(joint_idx).zip(weights) {
            let p = Vec3::from(*p);
            let reference = mesh_mat.transform_point3(p);
            lo = lo.min(reference);
            hi = hi.max(reference);
            let mut out = Vec3::ZERO;
            for k in 0..4 {
                if w[k] <= 0.0 {
                    continue;
                }
                let m = joint_mats.get(ji[k] as usize).copied().unwrap_or(Mat4::IDENTITY);
                out += w[k] * m.transform_point3(p);
            }
            devs.push(out.distance(reference));
        }
        devs.sort_by(|a, b| a.total_cmp(b));
        let extent = (hi - lo).max_element();
        rows.push(SkinDeviation {
            mesh: name,
            vertices: devs.len(),
            max_m: devs.last().copied().unwrap_or(0.0),
            median_m: devs.get(devs.len() / 2).copied().unwrap_or(0.0),
            extent_m: extent,
        });
    }
    rows
}

/// Load `units/<key>.glb`, spawn, and measure rest-pose and (optionally) clip-driven skinning.
pub fn probe_unit_glb(key: &str, clip: Option<&str>, seconds: f32) -> (Vec<SkinDeviation>, Vec<SkinDeviation>) {
    let mut app = headless_app();
    let handle: Handle<Gltf> = app
        .world()
        .resource::<AssetServer>()
        .load(format!("units/{key}.glb"));
    wait_loaded(&mut app, &handle);
    let (scene, clip_handle) = {
        let gltf = app.world().resource::<Assets<Gltf>>().get(&handle).unwrap();
        let clip_handle = clip.map(|c| {
            gltf.named_animations
                .get(c)
                .unwrap_or_else(|| panic!("{key}: clip {c} missing"))
                .clone()
        });
        (gltf.scenes[0].clone(), clip_handle)
    };
    app.world_mut().spawn(SceneRoot(scene));
    for _ in 0..60 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let rest = evaluate(&mut app);

    let Some(clip_handle) = clip_handle else {
        return (rest, Vec::new());
    };
    let (graph, node) = AnimationGraph::from_clip(clip_handle);
    let graph_handle = app.world_mut().resource_mut::<Assets<AnimationGraph>>().add(graph);
    let players: Vec<Entity> = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<AnimationPlayer>>()
            .iter(world)
            .collect()
    };
    assert!(!players.is_empty(), "{key}: no AnimationPlayer spawned by Bevy loader");
    for player in players {
        let world = app.world_mut();
        world.entity_mut(player).insert(AnimationGraphHandle(graph_handle.clone()));
        let mut p = world.get_mut::<AnimationPlayer>(player).unwrap();
        p.play(node).repeat().seek_to(seconds);
    }
    for _ in 0..6 {
        app.update();
    }
    let animated = evaluate(&mut app);
    (rest, animated)
}

/// Renderer-facing skin invariants on the mesh Bevy actually uploads: every vertex's four
/// weights sum to 1 and every weighted joint index addresses a joint of the bound skin.
/// A violation displaces the vertex by `(1 - sum) * world_position`, which is invisible at
/// the origin and a world-scale streak anywhere else.
fn skin_invariant_violations(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query::<(&Name, &Mesh3d, &SkinnedMesh)>();
    let data: Vec<(String, Handle<Mesh>, usize)> = query
        .iter(world)
        .map(|(n, m, s)| (n.to_string(), m.0.clone(), s.joints.len()))
        .collect();
    let meshes = world.resource::<Assets<Mesh>>();
    let mut violations = Vec::new();
    for (name, handle, joint_count) in data {
        let Some(mesh) = meshes.get(&handle) else {
            violations.push(format!("{name}: mesh asset not loaded"));
            continue;
        };
        let (Some(VertexAttributeValues::Uint16x4(joint_idx)), Some(VertexAttributeValues::Float32x4(weights))) = (
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX),
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT),
        ) else {
            violations.push(format!("{name}: skinned mesh without JOINT_INDEX/JOINT_WEIGHT attributes"));
            continue;
        };
        let mut bad_sum = 0usize;
        let mut min_sum = f32::MAX;
        let mut out_of_range = 0usize;
        for (ji, w) in joint_idx.iter().zip(weights) {
            let sum: f32 = w.iter().sum();
            min_sum = min_sum.min(sum);
            if (sum - 1.0).abs() > 1e-3 {
                bad_sum += 1;
            }
            if (0..4).any(|k| w[k] > 0.0 && ji[k] as usize >= joint_count) {
                out_of_range += 1;
            }
        }
        if bad_sum > 0 {
            violations.push(format!(
                "{name}: {bad_sum}/{} vertices with weight sum != 1 (min {min_sum:.4})",
                weights.len()
            ));
        }
        if out_of_range > 0 {
            violations.push(format!("{name}: {out_of_range} vertices reference joints outside the skin ({joint_count} joints)"));
        }
    }
    violations
}

fn unit_glb_keys() -> Vec<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/units");
    let mut keys: Vec<String> = std::fs::read_dir(dir)
        .expect("assets/units")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            (path.extension().and_then(|x| x.to_str()) == Some("glb"))
                .then(|| path.file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    keys.sort();
    keys
}

/// Export gate: every shipped unit GLB must satisfy the renderer's skinning invariants once
/// loaded by Bevy. Fails with the offending file/mesh so the export pipeline can be fixed.
#[test]
fn unit_glbs_satisfy_renderer_skin_invariants() {
    let mut failures = Vec::new();
    for key in unit_glb_keys() {
        let mut app = headless_app();
        let handle: Handle<Gltf> = app
            .world()
            .resource::<AssetServer>()
            .load(format!("units/{key}.glb"));
        wait_loaded(&mut app, &handle);
        let scene = app.world().resource::<Assets<Gltf>>().get(&handle).unwrap().scenes[0].clone();
        app.world_mut().spawn(SceneRoot(scene));
        for _ in 0..60 {
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        for violation in skin_invariant_violations(&mut app) {
            failures.push(format!("{key}.glb: {violation}"));
        }
    }
    assert!(
        failures.is_empty(),
        "unit GLBs violate renderer skin invariants (re-export with tools/monster_pack/export_monster_fbx2gltf.py):\n  {}",
        failures.join("\n  ")
    );
}

fn fmt_rows(label: &str, rows: &[SkinDeviation]) -> String {
    let mut s = format!("  [{label}]\n");
    for r in rows {
        s.push_str(&format!(
            "    {:<28} verts={:<6} max={:7.3}m median={:6.3}m extent={:5.2}m\n",
            r.mesh, r.vertices, r.max_m, r.median_m, r.extent_m
        ));
    }
    s
}

#[test]
#[ignore = "diagnostic: cargo test --features dev --lib skinning_runtime_report -- --ignored --nocapture"]
fn skinning_runtime_report() {
    let cases: &[(&str, &str)] = &[
        ("cavecrawler", "CrawlForward"),
        ("human_male", "Idle"),
        ("muscomorph", "IdleBreathe"),
        ("bufomorph", "IdleLookAround"),
    ];
    for (key, clip) in cases {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("assets/units/{key}.glb"));
        if !path.is_file() {
            continue;
        }
        let (rest, anim) = probe_unit_glb(key, Some(clip), 0.5);
        println!("== {key} ({clip} @0.5s)\n{}{}", fmt_rows("rest", &rest), fmt_rows("animated", &anim));
    }
}
