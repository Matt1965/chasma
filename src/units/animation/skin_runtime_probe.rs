//! Dev-only live skinning probe.
//!
//! For each spawned unit, evaluates linear-blend skinning with the entity `GlobalTransform`s the
//! GPU consumes (`joint_global * inverse_bindpose`) and appends a short report to
//! `logs/dev_startup.log` (summary line per unit; per-mesh/joint detail only when a mesh deviates
//! from its rest placement by more than [`DETAIL_THRESHOLD_M`]). Also reports the renderer skin
//! invariant (weights sum to 1, joint indices in range) because a violation there displaces
//! vertices by `(1 - sum) * world_position`, which looks like world-scale streaks in game while
//! being invisible in any origin-centred check.

use std::collections::{HashMap, HashSet};

use bevy::mesh::VertexAttributeValues;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;

use crate::logging::{DEV_STARTUP_LOG_PATH, append_log_block};
use crate::units::components::{UnitRenderEntity, UnitSceneRoot};
use crate::world::UnitId;

const SESSION_HEADER: &str = "# unit skin probe";
/// Frames after skinned meshes appear for the second sample (first clip is playing by then).
const LATE_SAMPLE_FRAMES: u32 = 60;
/// Per-mesh deviation above which joint-level detail is logged.
const DETAIL_THRESHOLD_M: f32 = 0.25;

#[derive(Default)]
pub struct SkinProbeLedger {
    frames_with_skin: HashMap<UnitId, u32>,
    done: HashSet<UnitId>,
}

struct MeshReport {
    name: String,
    vertices: usize,
    joints: usize,
    max_dev_m: f32,
    bad_weight_sum: usize,
    min_weight_sum: f32,
    out_of_range_joints: usize,
    worst_joints: Vec<(usize, f32)>,
    joint_entities: Vec<Entity>,
}

#[allow(clippy::too_many_arguments)]
pub fn probe_live_unit_skinning(
    mut ledger: Local<SkinProbeLedger>,
    roots: Query<(Entity, &UnitRenderEntity, &GlobalTransform), With<UnitSceneRoot>>,
    children: Query<&Children>,
    names: Query<&Name>,
    skinned: Query<(&Name, &Mesh3d, &SkinnedMesh, &GlobalTransform)>,
    globals: Query<&GlobalTransform>,
    meshes: Res<Assets<Mesh>>,
    ibps: Res<Assets<SkinnedMeshInverseBindposes>>,
    time: Res<Time>,
) {
    for (root, marker, root_global) in &roots {
        let unit_id = marker.unit_id;
        if ledger.done.contains(&unit_id) {
            continue;
        }
        let mut descendants = Vec::new();
        let mut stack = vec![root];
        while let Some(e) = stack.pop() {
            descendants.push(e);
            if let Ok(kids) = children.get(e) {
                stack.extend(kids.iter());
            }
        }
        if !descendants.iter().any(|e| skinned.contains(*e)) {
            continue; // scene not instanced yet
        }
        let frames = ledger.frames_with_skin.entry(unit_id).or_insert(0);
        *frames += 1;
        let sample = match *frames {
            1 => "spawn",
            f if f == LATE_SAMPLE_FRAMES => "animated",
            _ => continue,
        };

        let mut reports = Vec::new();
        for entity in &descendants {
            let Ok((mesh_name, mesh3d, skin, mesh_global)) = skinned.get(*entity) else {
                continue;
            };
            let (Some(mesh), Some(ibp)) = (meshes.get(&mesh3d.0), ibps.get(&skin.inverse_bindposes)) else {
                continue;
            };
            let (
                Some(VertexAttributeValues::Float32x3(pos)),
                Some(VertexAttributeValues::Uint16x4(ji)),
                Some(VertexAttributeValues::Float32x4(wt)),
            ) = (
                mesh.attribute(Mesh::ATTRIBUTE_POSITION),
                mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX),
                mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT),
            )
            else {
                continue;
            };
            let joint_mats: Vec<Mat4> = skin
                .joints
                .iter()
                .zip(ibp.iter())
                .map(|(j, inv)| globals.get(*j).map(|g| g.to_matrix() * *inv).unwrap_or(Mat4::IDENTITY))
                .collect();
            let mesh_mat = mesh_global.to_matrix();
            let mut report = MeshReport {
                name: mesh_name.to_string(),
                vertices: pos.len(),
                joints: skin.joints.len(),
                max_dev_m: 0.0,
                bad_weight_sum: 0,
                min_weight_sum: f32::MAX,
                out_of_range_joints: 0,
                worst_joints: Vec::new(),
                joint_entities: skin.joints.clone(),
            };
            let mut per_joint_max = vec![0.0f32; skin.joints.len()];
            for ((p, idx), w) in pos.iter().zip(ji).zip(wt) {
                let p = Vec3::from(*p);
                let reference = mesh_mat.transform_point3(p);
                let sum: f32 = w.iter().sum();
                report.min_weight_sum = report.min_weight_sum.min(sum);
                if (sum - 1.0).abs() > 1e-3 {
                    report.bad_weight_sum += 1;
                }
                let mut out = Vec3::ZERO;
                let mut dominant = (0usize, -1.0f32);
                for k in 0..4 {
                    if w[k] <= 0.0 {
                        continue;
                    }
                    let j = idx[k] as usize;
                    if j >= joint_mats.len() {
                        report.out_of_range_joints += 1;
                        continue;
                    }
                    out += w[k] * joint_mats[j].transform_point3(p);
                    if w[k] > dominant.1 {
                        dominant = (j, w[k]);
                    }
                }
                let d = out.distance(reference);
                report.max_dev_m = report.max_dev_m.max(d);
                per_joint_max[dominant.0] = per_joint_max[dominant.0].max(d);
            }
            let mut worst: Vec<(usize, f32)> = per_joint_max.into_iter().enumerate().collect();
            worst.sort_by(|a, b| b.1.total_cmp(&a.1));
            report.worst_joints = worst.into_iter().take(4).filter(|(_, d)| *d >= DETAIL_THRESHOLD_M).collect();
            reports.push(report);
        }

        let worst_mesh = reports.iter().map(|r| r.max_dev_m).fold(0.0f32, f32::max);
        let bad_weights: usize = reports.iter().map(|r| r.bad_weight_sum).sum();
        let mut block = format!(
            "unit `{}` {sample} t={:.1}s at {:.1?} meshes={} max_dev={worst_mesh:.3}m weight_sum_violations={bad_weights}",
            unit_id.raw(),
            time.elapsed_secs(),
            root_global.translation(),
            reports.len(),
        );
        for r in reports.iter().filter(|r| r.max_dev_m > DETAIL_THRESHOLD_M || r.bad_weight_sum > 0 || r.out_of_range_joints > 0) {
            block.push_str(&format!(
                "\n  mesh `{}` verts={} joints={} max_dev={:.3}m weight_sum<>1: {} (min {:.4}) out_of_range_joint_refs={}",
                r.name, r.vertices, r.joints, r.max_dev_m, r.bad_weight_sum, r.min_weight_sum, r.out_of_range_joints
            ));
            for (j, dev) in &r.worst_joints {
                let joint = r.joint_entities[*j];
                let name = names.get(joint).map(|n| n.as_str()).unwrap_or("<unnamed>");
                let global_t = globals.get(joint).ok().map(|g| g.translation());
                block.push_str(&format!("\n    joint[{j}] `{name}` dev={dev:.3}m global_t={global_t:.2?}"));
            }
        }
        append_log_block(DEV_STARTUP_LOG_PATH, SESSION_HEADER, &block);
        if sample == "animated" {
            ledger.done.insert(unit_id);
        }
    }
}
