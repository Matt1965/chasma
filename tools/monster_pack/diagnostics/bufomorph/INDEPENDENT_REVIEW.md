# Asset import and runtime presentation: independent review

Date: 2026-10-11. Branch `feature/monster-import-fidelity`. Bevy **0.18.1** (`Cargo.lock`).
Supersedes `DEFECT_REPORT.md` (its two conclusions are refuted below).

Scope: Bufomorph (U-0029) idle mouth and attack flip, then the whole import/presentation
pipeline. Analysis, controlled experiments and an implementation map only. No production
rewrite was performed; nothing was merged to `main`.

Working data (outside the repo): `C:\BevyFiles\Temp\bufo_diag\`. Copies of the evidence used
below are in `conversion_paths/` next to this file. Reproducible scripts are in
`tools/monster_pack/experimental/`.

---

## 1. What was actually tested

### 1.1 Build that produced "no observable improvement"

| Question | Finding | Evidence |
|---|---|---|
| Which exe ran? | The only `chasma.exe` on disk is `C:\BevyFiles\chasma-agent-c\target\debug\chasma.exe` (built 10/11 11:12:54, after commit `5fa523ac` at 11:04). `C:\BevyFiles\chasma\target\debug` has **no** exe (cleaned by `cargo clean -p chasma` during the earlier pass). | `Get-ChildItem` over the four worktrees' `target/debug/chasma.exe` |
| Which branch/commit? | `chasma-agent-c` was on `agent-c/monster-pack-import @ 67850963`, not `feature/monster-import-fidelity @ 5fa523ac`. | `git -C C:\BevyFiles\chasma-agent-c log -1` |
| Which GLB did it load? | Bevy asset root resolves from `BEVY_ASSET_ROOT` → `CARGO_MANIFEST_DIR` (compile-time, per worktree) → exe dir. The agent-c binary therefore loaded `chasma-agent-c\assets\units\bufomorph.glb` = **11,669,176 B** (old workaround export), not the branch's 11,840,528 B faithful export. | file sizes / SHA of both GLBs |
| Which workbook? | `dev_design_workbook_path()` = `env!("CARGO_MANIFEST_DIR")` (`src/data_import/paths.rs:9`), so the agent-c workbook: **no** `Upper Body Split Bone` for `bufomorph`, **no** Z=90 catalog correction. | workbook rows compared |

**Ruling:** the instruction "run from `C:\BevyFiles\chasma`" was correct as a path, but the
owner's run was almost certainly the agent-c binary (the only one that existed). The fix in
`5fa523ac` was **not consumed** by the running game. The dev window title
(`src/dev/build_identity.rs`) shows `branch @ sha [DIRTY]`; it is the cheapest way to confirm
this and should be read on every manual test.

**But:** this mismatch is not the reason the defects persist. Even with the correct binary and
workbook, the `5fa523ac` change would not have fixed either defect, because both root causes are
in the GLB itself (section 2). The mismatch only explains why the owner saw literally zero change.

### 1.2 Resolved configuration on the branch (what *would* have been tested)

| Item | Value | Source |
|---|---|---|
| Unit | `U-0029` bufomorph → `assets/units/bufomorph.glb` | Units sheet via `gltf_asset_path_for_definition` |
| Profile | `bufomorph`: idle `IdleLookAround`, walk/run `Run`, death `Death` | Animation Profiles row 30 |
| Weapon | `weapon_bufomorph_primary` → `animation_key = BiteAttack` | Weapons sheet |
| Split bone (in `5fa523ac`) | `Bufomorph_ Spine` — it **is** a real animation target in the GLB, so masked mode would have engaged | `configure_unit_animation_layering` |
| Split bone (now) | **cleared** (this pass) → `FullBodyExclusive` | workbook |
| Catalog correction | Y=180, Z=90 | Units sheet |

A dev-only diagnostic was added to `src/units/animation/skeleton.rs` so that none of the above
has to be inferred again: on first configuration of each unit definition it logs
`unit animation identity: unit ... glb <path> bytes=N fnv1a64=... profile ... idle ... attack ...`
and every layering decision logs `unit animation layering: ... split_bone ... targets=N upper=N lower=N mode=...`.
A split bone that matches zero animation targets now falls back to `FullBodyExclusive` with a
one-shot warning instead of silently producing an empty mask.

---

## 2. Bufomorph: independently located failures

Three-way comparison in equivalent spaces (script: `experimental/compare_paths.py`,
output: `conversion_paths/compare_paths.json`):

- **Unity prefab** `Buformorph_Red.prefab` — independent reference, parsed from YAML (not the
  Blender importer). LH Y-up; compared through mirror-invariant quantities.
- **FBX2glTF direct** outputs of `SK_Buformorph.FBX` and `Buformorph@*.FBX` — same converter,
  same space, no Blender.
- **Shipped** `assets/units/bufomorph.glb` — Blender merge path (`monster_export_core.py`).

### 2.1 Rest skeleton

Joint distances (metres), mirror-invariant:

| Pair | Unity | FBX2glTF SK | Shipped |
|---|---|---|---|
| Head ↔ JawUpper | 0.1506 | 0.1506 | 0.1506 |
| Pelvis ↔ Head | 0.4986 | 0.4986 | 0.4986 |
| `Bufomorph_` (hip) ↔ Pelvis | **0.0** | **0.0** | **0.3988** |
| `Bufomorph_` ↔ Head | 0.4986 | 0.4986 | **0.8960** |

The shipped rest skeleton has an extra 0.3977 m translation inserted on `Bufomorph_ Pelvis`
(`T=[0,0.3977,0.0292]`, rotation 120° away from the FBX2glTF/Unity value). FBX2glTF and Unity
agree to 1e-7. This is a Blender round-trip alteration of the rest pose, not source data. The
inverse bind matrices still match the mesh (the creature is not exploded), but every clip is now
evaluated against a rest skeleton that differs from the one the clips were authored on.

### 2.2 Orientation

Bind-pose world axes (`orientation_check`):

| | Unity | FBX2glTF SK | Shipped |
|---|---|---|---|
| spine axis (Pelvis→Head) | (0, 0.98, 0.18) | (0, 0.98, 0.18) | **(0.98, 0, 0.18)** |
| snout (Head→JawUpper) | (0, 0.51, 0.86) | (0, 0.51, 0.86) | (0.51, 0, 0.86) |

FBX2glTF SK is already **upright, Y-up, facing +Z**. The shipped file is rolled 90° so that
"up" is +X. That roll is `apply_fbx2gltf_scene_root_x90` (`glb_fbx2gltf_scene_fix.py`), a
post-export step that exists to restore an armature rotation Blender's *own* FBX importer would
have added — but on this path the SK came through FBX2glTF, which never added it. The catalog
**Z=90** correction compensates a conversion defect. With a correct GLB only **Y=180** is needed
(Chasma `MODEL_FORWARD_AXIS = NEG_Z`, source faces +Z).

### 2.3 Attack flip — root cause: 100× translation

`BiteAttack`, `Bufomorph_` (hip) translation delta t=0 → t=0.35 s:

| Path | Delta |
|---|---|
| FBX2glTF clip | **0.2726 m** |
| Shipped GLB | **27.259 m** |

Exactly 100×. `IdleLookAround` shows the same factor (0.0040 vs 0.4512). Pack-wide audit
(`experimental/audit_joint_translation_scale.py --no-rm assets/units/*.glb`): **30 of 32**
pack creatures have a joint translation excursion 15–112× larger than the whole rest skeleton
(e.g. `clypeosaurus Death` body 646 m; `bufomorph TongueAttack` Tongue6 57.6 m). The two
non-pack rigs (`human_*`, `fox`) and `cavecrawler` (converted by a different path) are normal.
Including `_RM` clips the worst case is `scyver FlyForwardFast_RM root` 3200 m.

Mechanism (`monster_export_core.py`): each clip FBX is imported with Blender's FBX importer
(`FBX_SCALE = 1.0`; the pack FBX is authored in cm, so the armature object arrives with scale
0.01 and pose-bone location fcurves in cm). `apply_object_scale` runs `transform_apply(scale)`
on the *object*, which rescales armature/mesh data but **not** the action's fcurves. The action
(now in cm) is then pushed onto the destination armature's NLA (`push_merged_actions_to_nla`),
whose bone space is metres → every translation key is 100× too large. Rotation fcurves are
unit-less and survive, which is why the body *articulates* plausibly while the hip flies away.
Because a Blender action stores pose-*basis* values relative to the destination armature's rest
pose, the same step also discards the clip rig's rest pose (§2.4).

Consequences for the previous diagnosis:

- The attack flip is **presentation-only**: sim position is correct; the skinned mesh is flung
  ~27 m along the hip's channel, rotated by the Z=90 correction into −Y → "under the world".
- Masking the lower body (`5fa523ac`) hid the flung hip while keeping the upper-body rotations.
  That is **symptom suppression** of a conversion defect, and it also removed the authored
  lunge. The split bone has been **withdrawn** from the workbook in this pass. Masked layering
  stays available as a gameplay tool for rigs where it is appropriate (humans).
- The "source FBX probe" values previously cited (`local_loc ≈ [5.34, 0, 31.82]`) were
  Blender-imported cm values, not an independent reference.

### 2.4 Open mouth — root cause: reference pose dropped

Rest rotation of the animation FBX (FBX2glTF clip-file node defaults) vs SK bind, both clips:

| Bone | Angle (deg) | Translation delta |
|---|---|---|
| Bufomorph_JawUpper | **35.0** | 0.045 m |
| Bufomorph_JawLeft / JawRight | **58.4** | 0 |
| Bufomorph_Tongue1 | **21.4** | 0 |
| Spine / Head | 5.0 | 0 |
| `Bufomorph_` (hip) | 0 | 0.113 m |

The animation FBX files carry their own rig whose rest pose is the **closed-mouth** pose. The
SK FBX bind pose is the **open-mouth** modelling pose. Unity (absolute TRS playback, `.meta`
shows generic rig, `useFileScale=1`) poses every joint from the clip, so joints the clip does not
key are left at the clip rig's rest = closed. The Blender merge turns each clip into pose
*deltas relative to the destination armature's rest* (`merge_action`), so the 35°/58° offsets
vanish and un-keyed jaw joints sit at the SK bind (open) pose. This is why the mouth is open
in every clip, including death. It is a conversion defect, not authoring.

Secondary: FBX2glTF drops a channel when it is constant and equal to the node default *of that
file*. A glTF-level merge therefore must synthesize a constant channel for every un-keyed joint
whose clip-rig default differs from the SK bind (prototype does this; 6 channels for idle).

### 2.5 Prototype verification (local, Blender-free merge)

`experimental/merge_fbx2gltf_clips.py` merged FBX2glTF SK + three FBX2glTF clips at the glTF
level. Rendered via the existing headless path (`blender_render_compare.py`, untextured because
FBX2glTF SK carries no texture bind):

| | Shipped | Prototype |
|---|---|---|
| Idle t=0 | `conversion_paths/shipped_IdleLookAround_t0.00.png` — rolled on side, jaw open, tongue out | `proto_IdleLookAround_t0.00.png` — upright, **mouth closed** |
| BiteAttack t=0.35 | `shipped_BiteAttack_t0.35.png` — mesh is a speck ~27 m away | `proto_BiteAttack_t0.35.png` — body intact, lunging |
| Audit ratio | 25.6 (RED) | **0.2** (ok); worst joint `Bufomorph_` 0.36 m in BiteAttack |

Status: both defects are **located and reproduced with a corrective path demonstrated
offline**. Neither is **fixed in the game**: the shipped GLB is unchanged and no in-game run of
the prototype has occurred. See section 5 for the owner procedure.

### 2.6 Unresolved

- Unity-side playback of the clips was not run (no Unity on this machine); the reference is
  prefab rest TRS + import settings + the conversion-invariant checks above.
- Prefab parser skips transforms without all four fields; `L HorseLink` / `L Foot` were not
  cross-checked (irrelevant to the two defects).
- Root-motion policy for `_RM` clips (which legitimately move `root`) is an owner decision
  (section 6); the prototype copies them untouched.
- Blender-free material binding is designed (section 4) but not prototyped.

---

## 3. Pipeline map and failure boundaries

### 3.1 Rigged pack creatures (32 of 33 unit GLBs)

```
Unity pack (cm, LH Y-up, SK_X.FBX + X@Clip.FBX + .mat/.png)
  └─ tools/monster_pack/monster_export_core.py  (Blender 4.2 headless)
       import_sk_armature: Blender FBX import → on failure FBX2glTF→glTF import (fallback)   [REST POSE ALTERED §2.1]
       import_fbx per clip (FBX_SCALE=1.0, cm) + apply_object_scale (does not rescale fcurves)  [UNITS]
       merge_action (rename) → push_merged_actions_to_nla onto destination armature            [100× TRANSLATION §2.3, REFERENCE POSE LOST §2.4]
       export_scene.gltf
       unity_materials.apply_textures_to_meshes (heuristic: first *_BaseColor, roughness 0.45)
       finish_glb_post_export: rebase_gltf_animation_starts_to_zero (valid repair)
                               apply_fbx2gltf_scene_root_x90 (fallback only)                 [ROLLS MODEL §2.2]
       <out>.export_manifest.json
  └─ assets/units/<name>.glb  (run_faithful_batch_export.ps1 regenerated all 32 with the defects above; commit e40effa3)
  └─ Chasma Design.xlsx  Units (gltf path, rotation correction, ModelComposition), Animation Profiles, Weapons
  └─ runtime: src/units/animation/{assets,discovery,playback,skeleton,validation}.rs
              src/world/asset_sizing/composition.rs::unit_visual_rotation (catalog correction on UnitSceneRoot)
              src/data_import/asset_sizing/bounds.rs::measure_glb_source_bounds (metric sizing, ADR-097/126-128)
```

Ownership today: conversion, orientation, units and animation prep are all inside one Blender
script; catalog corrections compensate for its output. Failure behavior: silent — the manifest
reports `complete=true` for files with 100× translations; no stage compares output against the
source.

### 3.2 Humans (`human_male/female.glb`, appearance + equipment morphs)

Authored externally (no export tooling in repo; `docs/animation-authoring.md` says "author in
DCC"). Runtime: `src/units/appearance_presentation/sync.rs::sync_unit_appearance_morphs`,
`src/units/equipment_presentation/{morph_sync,skinned_overlay,bones}.rs`. Audit clean
(ratio 0.4–0.5, pelvis travel in `Death` = authored fall). Split-bone masked layering is
appropriate here (`spine_02`).

### 3.3 Equipment (`assets/items/equipment/human_*/…glb`, weapons)

Skinned overlays bound to the human skeleton by bone name, `clearance_validation.json` for
morph clearance. External authoring; no converter in repo.

### 3.4 Buildings / doodads (static)

`assets/buildings/catalog.ron`, `assets/doodads/*/`. Sizing: ADR-097 metric sizing +
ADR-126/127/128 transform catalog/composition; footprints: `src/world/occupancy/bake.rs`
(ADR-080). No animation path; not implicated.

### 3.5 Materials / textures

`unity_materials.py` guesses from filenames (ignores `.mat`, Occlusion; fixed roughness;
MetallicSmoothness R→metallic). Works for Bufomorph red tint; not data-driven.

### 3.6 Legacy stripping / previously generated assets

`glb_sanitize_animations.py --strip-child-channels` is opt-in only (correct). The removed
workarounds listed in `IMPORT_FIDELITY.md` were all compensations for §2.1–2.4. **Every pack GLB
in `assets/units/` currently carries the 100× defect** and will need regeneration under the
target process; `cavecrawler` (different converter) does not.

---

## 4. Recommended target process

Options assessed:

| Option | Fidelity | Automation | Coverage | Deps | Migration | Diagnostics |
|---|---|---|---|---|---|---|
| A. Keep Blender merge, repair units + reference pose inside Blender | Possible but fragile (3 known alterations: rest pose, units, reference) | existing | pack only | Blender | small code, full regen | opaque (Blender internals) |
| **B. FBX2glTF for SK and clips, glTF-level merge by node name, glTF-level materials** | **Validated: matches Unity rest to 1e-7, clip deltas exact, mouth closed, upright** | pure Python + one exe | pack (any FBX with per-clip files) | FBX2glTF, Python, Pillow (textures) | new ~300-line tool, full regen | transparent, every step inspectable JSON |
| C. Export from Unity (glTF exporter) | High | manual per asset | needs Unity on authoring machine | Unity | per-asset manual | low |

**Recommendation: B.** Remove Blender from the skeleton/animation path entirely. Blender
remains an optional *diagnostic renderer* (`blender_render_compare.py`) and a fallback for
assets FBX2glTF cannot read.

Ownership under B:

| Concern | Owner | Rule |
|---|---|---|
| Source conversion (units, handedness) | FBX2glTF | one converter for SK and clips; no post-hoc scale/rotate steps |
| Orientation | none needed in conversion; catalog `rotation correction` only for *authored* forward-axis differences (Y=180 for +Z-facing packs) | if a catalog correction is not a multiple of 90° about Y, it is compensating a defect |
| Animation prep (merge, un-keyed joint fill, time rebase) | `tools/monster_pack/gltf_merge.py` (promoted from `experimental/merge_fbx2gltf_clips.py`) | absolute TRS copy by node name; constant fill from clip-rig defaults; no stripping |
| Root motion | runtime policy (ADR-074/076 scope): catalog selects `X` vs `X_RM`; `root` channel handling is a gameplay decision, never deleted in conversion | owner decision §6 |
| Materials | glTF-level material step reading Unity `.mat` (Tint selection, Metallic/Smoothness repack) | replaces filename heuristics |
| Catalog adaptation (ModelComposition offset, profile clips, split bone) | workbook | gameplay adaptation, not repair |
| Runtime playback | `src/units/animation/*` unchanged; identity/layering log is the verification hook | — |

Gameplay adaptation vs import repair: a catalog rotation of Y=180, a ground offset, and the
choice of clip names are adaptation. A Z=90 roll, a split bone added to hide a flying hip, or a
tongue collapse are repairs of conversion defects in the wrong place and must not exist.

---

## 5. Validation design

Automatable (CI-able, Python only):

1. **Scale sanity** — `audit_joint_translation_scale.py --no-rm` ratio ≤ 1.0 for every unit GLB
   (currently 30 failures; becomes the regression gate).
2. **Rest fidelity** — `compare_paths.py`: joint distances shipped vs FBX2glTF SK vs Unity
   prefab within 1e-4 m; rest rotation difference 0° for all skin joints.
3. **Clip fidelity** — per clip, per joint: sampled translation/rotation in shipped GLB equals
   FBX2glTF clip value (absolute) at matching times within 1e-4 m / 0.1°.
4. **Structural completeness** — every clip in the pack folder present by name; channel count
   per clip ≥ FBX2glTF clip channels + filled constants; `export_manifest.json complete=true`.
5. **Orientation** — bind-pose spine axis within 5° of +Y and snout within 5° of ±Z before any
   catalog correction; catalog correction allowed only as k·90° about Y.
6. **Runtime config** — `cargo test --features dev --lib animation::validation` plus the
   identity log: `bytes`/`fnv1a64` of the loaded GLB must equal the committed file.

Visual (owner):

7. Headless renders (`blender_render_compare.py`) of idle t=0, attack peak, death end, for each
   regenerated creature — mouth/limb pose check against the Unity demo scene screenshots.
8. In-game: idle, attack on a target, recovery, run, death, on Bufomorph and one unrelated
   pack creature, via the dev preview (`src/dev/tools/preview_model.rs`) which uses the real
   playback path.

### Owner procedure for the compiled diagnostic (this branch)

1. In `C:\BevyFiles\chasma`: `git status` shows branch `feature/monster-import-fidelity`.
2. `cargo run --features dev` **from that directory** (this builds `C:\BevyFiles\chasma\target\debug\chasma.exe`).
3. Window title must read `Chasma DEV — feature/monster-import-fidelity @ <sha>`; if it shows
   `agent-c/...`, the wrong binary is running.
4. Spawn a Bufomorph; the log shows
   `unit animation identity: unit \`U-0029\` ... glb assets/units/bufomorph.glb bytes=11840528 fnv1a64=...`
   and `unit animation layering: ... split_bone \`\` ... mode=FullBodyExclusive`.
5. Expected behavior on the **current** shipped GLB: mouth still open, attack still flings the
   body (the GLB is unchanged; this run only proves the runtime consumed the intended data).
6. For the prototype: copy `C:\BevyFiles\Temp\bufo_diag\bufomorph_gltf_merge_prototype.glb`
   over `assets/units/bufomorph.glb` **in a scratch copy only**, set catalog rotation to Y=180,
   Z=0, repeat step 4; expected: upright, mouth closed in idle, attack lunge in place
   (untextured — material step not yet built). Do not commit that file.

---

## 6. Implementation map (ordered)

| # | Step | Files | Acceptance |
|---|---|---|---|
| 1 | **Promote glTF-level merge to a tool**: `tools/monster_pack/gltf_merge.py` from `experimental/merge_fbx2gltf_clips.py`; add FBX2glTF invocation wrapper (`fbx2gltf_convert.py`: SK + every `@Clip.FBX` in a pack folder → temp dir), time rebase, manifest | new 2 files; `README.md` | Bufomorph regenerated; audit ratio ≤ 1; compare_paths rest distances match Unity to 1e-4; renders show closed mouth / in-place lunge |
| 2 | **glTF-level materials**: parse Unity `.mat` (tint selection by name), bind BaseColor/Normal, repack MetallicSmoothness (R→metallic, A→1-roughness) with Pillow, write images into GLB | `tools/monster_pack/gltf_materials.py`; retire `unity_materials.py` | textured Bufomorph render matches Unity demo screenshot |
| 3 | **Validation gate**: `tools/monster_pack/validate_pack_glb.py` running checks 1–5 of §5; wire into `run_faithful_batch_export.ps1` | new file + ps1 | all checks pass for Bufomorph; fail for current shipped file |
| 4 | **Catalog**: Bufomorph rotation correction Y=180, Z=0; `ModelComposition` offset re-measured; split bone stays empty | `Chasma Design.xlsx` | in-game upright, feet on ground |
| 5 | **Regenerate the pack** (31 remaining creatures) through steps 1–3; reset their Z/X corrections that compensated the roll | `assets/units/*.glb`, workbook | audit 0 RED; spot-render 5 creatures |
| 6 | **Retire** `apply_fbx2gltf_scene_root_x90`, Blender merge path (`monster_export_core.merge_action`, `blender_export_monster.py`) to `legacy/` or delete; update `IMPORT_FIDELITY.md` repair map | tools/monster_pack | README describes one path |
| 7 | **Runtime**: keep identity/layering log; add a dev `warn!` when a unit's catalog rotation correction is not k·90° about Y (defect compensation smell) | `src/units/animation/skeleton.rs`, `src/world/asset_sizing/composition.rs` | — |

**First coherent step:** #1. It is self-contained, exercises the validated prototype end to end
on one creature, and produces the first file the acceptance checks can be run against. Nothing
else in the map is credible until #1 passes in-game.

Migration scope: 32 GLBs regenerated (binary churn in `assets/units/`), one workbook column
adjusted for ~32 rows, no runtime behavior change beyond diagnostics.

---

## 7. Owner decisions required

1. **Approve option B** (drop Blender from the skeleton/animation path; FBX2glTF + glTF-level merge).
2. **Root motion policy**: use in-place clips and let gameplay drive position (current), or
   consume `_RM` `root` channels as root motion (new runtime work, ADR-074/076 scope).
3. **Tint selection** per creature (Unity ships 5 `.mat` tints; currently "first BaseColor").
4. **Split bone for bufomorph stays empty** (withdrawn this pass) — confirm; masked layering
   remains for humans.
5. Whether `cavecrawler` (different converter, audit clean) is regenerated for consistency or
   left as is.

---

## Changes in this pass (branch only, not merged)

- `src/units/animation/skeleton.rs`: dev-only identity + layering diagnostics; empty-mask fallback.
- `Chasma Design.xlsx`: `bufomorph` `Upper Body Split Bone` cleared (workaround withdrawn).
- `tools/monster_pack/experimental/{merge_fbx2gltf_clips,compare_paths,audit_joint_translation_scale}.py`.
- `tools/monster_pack/diagnostics/bufomorph/{INDEPENDENT_REVIEW.md, conversion_paths/*, DEFECT_REPORT.md (superseded banner)}`.
- `assets/units/bufomorph.glb` **unchanged**. Neither Bufomorph defect is fixed in the game.
