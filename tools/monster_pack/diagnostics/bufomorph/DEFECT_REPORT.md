# Bufomorph runtime defects (U-0029)

> **SUPERSEDED (2026-10-11) by [`INDEPENDENT_REVIEW.md`](INDEPENDENT_REVIEW.md).**
> The conclusions below were refuted by the independent review:
>
> - The attack flip is **not** authored hip motion that needs upper-body masking. The shipped
>   `BiteAttack` hip translation is exactly **100x** the FBX2glTF conversion of the same clip
>   (27.26 vs 0.2726 m): a Blender merge unit defect. The "source FBX" probe values quoted below
>   are Blender-imported centimetre pose values, not an independent reference. The split-bone
>   workaround has been withdrawn from the workbook.
> - The open mouth is **not** source-authored. Unity prefab bind TRS and FBX2glTF clip data both
>   show a closed mouth; the Blender pose-delta retarget drops the animation-FBX rest pose
>   (JawUpper 35 deg, JawLeft/Right 58 deg, Tongue1 21 deg offsets from SK bind).
> - The "source" render used as evidence was blank (animation FBX files contain no mesh).
>
> Kept for history only. Do not act on the repair tables below.

Manual reproduction on faithful `assets/units/bufomorph.glb` with catalog rotation **Y=180°, Z=90°**.

## Attack flip (beneath world)

### Clip selected by gameplay

| Stage | Value |
|-------|--------|
| Weapon `weapon_bufomorph_primary` | `animation_key` = **`BiteAttack`** |
| Resolver | `resolve_attack_clip_name` → `BiteAttack` (no alternate variant) |
| Layering (before fix) | Profile **`bufomorph`** had **no** `upper_body_split_bone` → **`FullBodyExclusive`** |
| Playback | Entire `BiteAttack` clip on full body (including `Bufomorph_` hip/root motion) |

### Four-way comparison

| Tier | `BiteAttack` @ ~0.35s — `Bufomorph_` motion |
|------|-----------------------------------------------|
| **Pack source FBX** | `local_loc` ≈ `[5.34, 0, 31.82]` on `Bufomorph_` (`SOURCE_PROBE_BiteAttack_t0.35.json`) |
| **Faithful GLB** | Translation channel ≈ `[0, -3.41, 27.45]` (`CLIP_MOTION_SAMPLES.json`) — same authored lunge, export units |
| **Chasma single-clip** | Same GLB data; flip reproduces when full body plays attack without masking |
| **Gameplay** | Same clip + full-body exclusive → hip translation rotates with presentation correction → visual dive under terrain |

`BiteAttack_RM` carries the same class of body motion (not a separate in-place clip).

### Simulation vs presentation

- **Authoritative unit position** does not follow skeletal root motion today (no root-motion extraction).
- **Defect is presentation-only**: `UnitSceneRoot` catalog correction stays on the render entity; the **skinned hierarchy** moves with animated `Bufomorph_` channels.
- Idle / `IdleLookAround` keep `Bufomorph_` near bind pose; attack replaces lower body with lunge keys.

### Cause

**Playback configuration**, not a conversion regression: faithful import preserved pack hip motion on `BiteAttack`. Full-body exclusive playback applies that motion while Chasma keeps the entity fixed — incompatible with masked combat presentation used elsewhere.

### Fix (applied)

**`Animation Profiles` / `bufomorph`**: set **`Upper Body Split Bone`** = **`Bufomorph_ Spine`**.

- Locomotion (`IdleLookAround`, `Run`) → lower-body mask.
- `BiteAttack` → upper-body mask (jaw/spine/arms); hips/legs stay on locomotion.
- Channels are **not** stripped; ADR-075 masked layering.

**Runtime verification (one pass after pulling this branch):**

1. `cargo run --features dev` from `C:\BevyFiles\chasma` (reloads `Chasma Design.xlsx` split bone).
2. Spawn U-0029 Bufomorph; confirm idle is `IdleLookAround` (fluid body, open mouth unchanged).
3. Order attack on another unit; during windup/active phases confirm mesh stays above terrain (hips do not lunge underground).
4. After attack: idle resumes; walk (`Run`); kill for `Death`.
5. Optional control: spawn a masked creature (e.g. muscomorph) and bite once — shared playback unchanged.

**MANUAL RUNTIME VERIFICATION PENDING** until owner runs the steps above.

### Evidence files

- `CLIP_MOTION_SAMPLES.json`
- `SOURCE_PROBE_BiteAttack_t0.35.json`
- `renders/source_pack_BiteAttack_t0.35.png` vs `renders/checkpoint_03_faithful_BiteAttack_t0.35.png` (from prior checkpoint pass)

---

## Open mouth on idle

### In-game idle

| Field | Value |
|-------|--------|
| Profile `bufomorph` | **`IdleLookAround`** (not `IdleBreathe`) |
| Playback | Catalog preview + units use profile idle mapping |

### Jaw / tongue comparison @ t=0

| Tier | `Bufomorph_JawUpper` |
|------|----------------------|
| Pack `IdleLookAround` FBX | `world_quat` ≈ `[0.584, -0.399, -0.584, 0.399]` |
| Pack `IdleBreathe` FBX | **Identical** jaw world pose at t=0 |
| Faithful GLB `IdleLookAround` | Rotation ≈ `[-0.8601, -0.5101, …]` (matches `IdleBreathe` in GLB) |
| Runtime | Plays `IdleLookAround`; jaw keys match export (no extra blend on jaw) |

### Cause

**Authored source behavior.** Pack idle clips bind the jaw in an open pose. Conversion and runtime preserve that relationship; switching profile idle from `IdleBreathe` to `IdleLookAround` does not close the mouth.

### Status

**Not a defect** under import fidelity policy. Closing the mouth would be **intentional custom animation work** (new clip, morph, or rig edit) and needs a separate design owner decision.

### Comparison images

| PNG | Description |
|-----|-------------|
| `renders/source_pack_IdleLookAround_t0.png` | Pack FBX `IdleLookAround` @ 0s |
| `renders/checkpoint_03_faithful_IdleLookAround_t0.00.png` | Faithful GLB same clip/time |
| `renders/source_pack_IdleBreathe_t0.png` | Pack idle breathe (jaw matches look-around at bind) |

Automated checks: `bufomorph_idle_look_around_jaw_pose_matches_pack_authoring` in `validation.rs`.
