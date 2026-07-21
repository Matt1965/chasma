# ADR-126: Asset Transform Standardization (AT0 — Design)

## Status

Accepted (AT0 design lock). **AT1–AT3 implemented** — see ADR-127, ADR-128, ADR-129.

## Context

Chasma already has a metric sizing foundation (ADR-097 DT1–DT4):

- Catalog `AssetSizingDefinition` with desired meters, source bounds, baseline scale
- Offline GLB bounds measurement (`data-import` only)
- `FixedScale` / `AuthoringScale` (milliunits)
- Dev transform editing, gizmos, calibration export
- Instance scale on doodad/building placements (scene v8/v9)

The remaining player-visible problem is inconsistency:

- Some GLBs appear microscopic or enormous at runtime
- Dev scaling exists but is easy to treat as the *primary* size control
- Dual fields still exist for building pivot/yaw (`model_local_offset` vs `asset_sizing`, anchor yaw vs child rotation)
- Building **visual** baseline does not resize occupancy footprints (only instance uniform scale does)
- Production builds without `data-import` cannot re-measure GLBs; they rely on baked/missing sizing
- `FixedScale` clamps `0.05..=20.0`, so extreme import corrections fail quantization

AT0 designs how Chasma standardizes **meters as the authoring language** so the game does not depend on hand-correcting every asset after import. Implementation is a later AT phase series.

This ADR **extends** ADR-096–100; it does not discard them.

---

## 1. Where authoritative size lives

| Layer | Owns | Does not own |
|---|---|---|
| **Catalog definition** (`AssetSizingDefinition` on Building / Doodad / Unit defs) | Desired dimensions (meters), size policy, pivot/rotation **corrections**, baked baseline scale, baked source bounds | Per-instance placement |
| **Import pipeline** (`data-import`) | Measuring source bounds from GLB, computing baseline, writing reports, failing/warning on mismatch | Runtime authority |
| **Runtime instance records** | Placement pose: position, orientation, instance scale | Re-deriving import math |
| **Import cache / reports** | Diagnostics (`logs/asset_sizing_report.md`, migration state) | Gameplay truth |

### Recommendation

**Catalog definitions are authoritative for metric size.**

- Authors think in **desired meters** (chest ≈ 1.5 m wide, human ≈ 1.8 m tall, tree height authored).
- Import **derives** source bounds + baseline scale and **bakes** the resolved baseline into the catalog (or RON export) so runtime never requires GLB re-parse.
- Runtime records store only **instance** transform deltas relative to that baseline.
- Do **not** make ECS `Transform.scale` or ad-hoc Dev multipliers the source of truth.

Rejected alternatives:

- **Asset metadata alone** (sidecar next to GLB): useful as import input, but catalogs already own content identity and must remain the load path.
- **Runtime records as size authority**: would fork every instance and break rebuild/migration.
- **Import cache as sole authority**: not available in production without `data-import`; must bake into definitions.

---

## 2. Source model size (imported GLB dimensions)

### Default measurement pipeline (keep + harden)

Order of preference (already largely implemented in `measure_glb_source_bounds`):

1. **Explicit authored source dimensions** on the definition (manual override when mesh is unreliable)
2. **Named bounds node** (`source_bounds_node`, default `size_reference`) if present in the GLB
3. **Combined visible mesh AABB** (vertices × node transforms), excluding helper/collision/portal/gizmo/light/camera nodes
4. Structured failure → `MissingSizingData` / migration state (no silent `1.0`)

### Recommendations

| Source | Use for visual baseline? | Notes |
|---|---|---|
| Render mesh AABB | **Default** | Assumes GLB unit = meter at export; warn on non-identity root scale |
| Named `size_reference` node | **Preferred when authored** | Artists control the box; most stable |
| Explicit `SourceDimensions` | Override | For broken meshes or intentional proxies |
| Collision mesh | **Never for visual sizing** | Collision is a separate authored gameplay shape (meters) |
| Multi-scene GLB | Reject / require selection | Already rejected without selection |

**Unit mismatch detection:** retain and strengthen suspected cm/mm export warnings (e.g. source ≪ desired by ×50+). Prefer fixing desired meters or re-exporting the GLB over extreme baseline scales.

---

## 3. Metric standardization

### Mental model

```
Imported source size (meters, measured or explicit)
        ↓
Desired Chasma dimensions (meters, catalog)
        ↓
Baseline scale = desired / source   (definition-owned)
        ↓
Instance scale (placement; default 1.0)
        ↓
Presentation Transform.scale = baseline × instance
```

Authors and Dev Mode should primarily edit **desired meters** (and optionally instance %), not raw mysterious scale floats.

### Rules

- **One reference axis → uniform baseline** for units and buildings (no distortion).
- **Optional non-uniform baseline** for doodads only when all three desired axes are authored.
- **Explicit baseline scale** remains an escape hatch (XOR with desired dimensions — keep ADR-097).
- **Gameplay footprint / collision radii** are authored in meters independently, but must be **validated against** approximate final visual dimensions (topology/navigable checks already exist; AT work should make failures loud and actionable).

### Scale range

ADR-097’s `0.05..=20` clamp is a quantization safety net, not an authoring target. AT implementation should:

- Prefer correcting **desired meters** or **source measurement** so baseline stays near `1.0`
- Treat out-of-range baseline as an **import error**, not a silent clamp-to-1.0
- Allow documenting rare exceptions via explicit baseline only after review

Avoid “every object has arbitrary scale numbers” by making **meters** the Excel/Dev primary columns and treating scale as derived.

---

## 4. Definition vs instance

### Definition (shared by all instances)

| Concern | Field / concept |
|---|---|
| Default / target size | `desired_*_meters`, `size_reference_axis` |
| Import correction | `calculated_source_bounds`, `calculated_baseline_scale` (baked) |
| Origin / pivot correction | Single authoritative model-local offset (meters) |
| Rotation correction | Single authoritative correction (buildings: prefer yaw-only for navigable safety) |
| Scale policy | Uniform vs non-uniform allowed; min/max instance clamps |
| Gameplay size | Footprint / collision radii / unit `collision_radius_meters` (meters) |

### Instance (per world object)

| Concern | Field / concept |
|---|---|
| Placement | `WorldPosition` |
| Rotation / orientation | Building `Quat` / doodad `QuantizedOrientation` |
| User / Dev scaling | Building `uniform_scale`; doodad `AuthoringScale` |
| Not persisted | Gizmo preview, temporary editor scratch, live import measurement |

### Dual-field cleanup (design intent for implementation)

Today buildings have overlapping truths:

- `BuildingDefinition.model_local_offset` vs `asset_sizing.model_local_offset_meters`
- `model_yaw_correction_degrees` (anchor) vs `asset_sizing.rotation_correction` (child)

**AT design decision:** one definition-owned correction path under `asset_sizing` (or a single clearly named alias), with ADR-096 fields becoming deprecated mirrors until migration completes. No double application of yaw. Builtin barn offsets should migrate into catalog data, not remain code special cases.

---

## 5. Doodads vs buildings

| Rule | Doodads | Buildings |
|---|---|---|
| Baseline policy | Uniform by axis **or** non-uniform when all three desired dims set | **Uniform only** (reference axis) |
| Instance scale | Non-uniform allowed when policy says so | **Uniform only**; gated by `allow_instance_scale` / safety class |
| Distortion | Acceptable for props | **Forbidden** for navigable structures |
| Gameplay coupling | Collision ellipse/circle scales with baseline × instance XZ (ADR-098) | Occupancy footprint authored in meters; instance uniform scale resizes footprint (ADR-100); baseline must stay consistent with footprint via validation |
| Flexibility | Decorative / organic | Preserve portals, spaces, pathfinding assumptions |

Units: uniform baseline from height (or authored axis); collision radius remains authored meters and must be validated against visual height/width (do not silently leave robot collision at 1.0 while mesh is 0.02 m tall).

---

## 6. Collision, picking, occupancy, navigation

### Principle

**One metric truth per concern, composed from the same baseline × instance rules.**

| System | Scale source |
|---|---|
| Render | baseline × instance |
| Doodad collision / occupancy | baseline × instance XZ (yaw-only for ground footprint) — ADR-098 |
| Building occupancy | Footprint meters × **instance** uniform scale — ADR-100 |
| Building visual baseline | Must not silently diverge from footprint; import/validation enforces approximate match |
| Picking | Uses presentation / authored interaction; must track the same final size players see |
| Navigation / portals / spaces | Building topology scales with instance uniform scale only (ADR-100); not with ad-hoc visual offsets |

### Avoid

- Separate “visual scale” and “collision scale” knobs
- Model-local offset affecting occupancy (keep visual-only — ADR-097)
- `CollisionShape::None` / baked paths that ignore baseline while ellipse paths honor it (unify in AT implementation)

Building note: DT1 intentionally left footprint unscaled by **definition baseline**. AT should either:

1. **Preferred:** treat footprint meters as gameplay size and require visual desired dimensions ≈ footprint (validation), or  
2. Optionally, in a later AT phase, derive default footprint from desired dims when footprint is missing — without auto-distorting authored footprints.

---

## 7. Save / load

### Persist

- Instance placement: position, rotation/orientation, instance scale milliunits (already scene v8/v9)
- Catalog definitions (including baked baseline / desired meters) via normal content load — not per-scene re-measure

### Do not persist

- Live GLB AABB recomputation
- Temporary gizmo preview transforms
- Import-only diagnostic heatmaps
- Editor selection / camera-relative scratch

After load: rebuild presentation from definition baseline × instance scale; revalidate occupancy registration. If baseline missing (`MissingSizingData`), fail loudly in Dev and fall back safely in production with diagnostics — do not invent a second silent scale path.

---

## 8. Dev Mode exposure

Dev Mode should explain size in meters, not only scale sliders:

| Display | Meaning |
|---|---|
| Source size (W/H/D m) | Measured or explicit import bounds |
| Desired size (W/H/D m) | Catalog target |
| Baseline scale | Derived (or explicit) definition scale |
| Instance scale | Placement multiplier (default 1) |
| Final size (m) | `source × baseline × instance` (approx) |
| Footprint / collision (m) | Gameplay sizes for comparison |
| Corrections | Pivot offset (m), rotation correction |
| Migration state | Verified / migrated / missing / mismatch |

Editing priority in Dev:

1. Adjust **desired meters** (definition calibration export → Excel) for content-wide fixes  
2. Adjust **instance scale** for one-off placed objects  
3. Avoid raw Transform.scale as the mental model

Keep calibration CSV export (ADR-100); extend columns if needed so artists see source vs desired vs final.

---

## Catalog field recommendations

**Keep (primary):**

- `desired_width_meters` / `desired_height_meters` / `desired_depth_meters`
- `size_reference_axis`
- `source_bounds_node`
- `explicit_source_dimensions`
- `explicit_baseline_scale` (escape hatch)
- `calculated_source_bounds` / `calculated_baseline_scale` (baked)
- `model_local_offset_meters`, `rotation_correction`
- `migration_state`

**Deprecate / migrate (implementation phases):**

- Parallel `BuildingDefinition.model_local_offset` + `model_yaw_correction_degrees` as second truth
- Hardcoded builtin offset maps
- Relying on legacy unit `render_scale` / doodad `min_scale`/`max_scale` as size authority (retain as clamps only)

**Do not add:**

- Per-instance “import correction” on world records
- A second collision scale channel

---

## Runtime transform ownership

```
Definition.asset_sizing  →  baseline + corrections
Instance.placement       →  position, orientation, instance scale
Presentation (ECS)       →  composed Transform (never authoritative)
Occupancy / collision    →  meters from gameplay specs × agreed scale rules
```

Composition order remains ADR-097:

```
placement × instance rotation × definition rotation correction
  × baseline scale × instance scale × model-local offset (visual)
```

---

## Import pipeline (design)

```
GLB on disk
  → measure source bounds (node / AABB / explicit)
  → read desired meters from Excel/catalog
  → calculate baseline (policy by content type)
  → validate vs footprint / collision / topology
  → bake calculated_* into definition
  → emit AssetSizingReport
Runtime load
  → use baked baseline (no GLB parse required)
```

Production without `data-import` must ship **baked** sizing in catalogs. Missing bake = migration debt, not a new runtime measurer.

---

## Migration strategy

1. **Inventory:** sizing report for all buildings/doodads/units — flag MissingSizingData, out-of-range baseline, visual↔footprint mismatch, dual-offset cases.
2. **Author desired meters** for high-traffic content (chests, humans, core buildings, common trees) until baseline ≈ 0.5–2.0.
3. **Bake baselines** into shipped catalogs/RON.
4. **Unify correction fields** (single pivot/yaw path).
5. **Tighten validation** so new imports cannot ship with MissingSizingData in Dev.
6. **Only then** consider footprint auto-align helpers (optional, never silent overwrite of authored footprints).

Legacy scenes: instance scales remain valid; after content rebake, some Dev-placed extremes may need one-time recalibration.

---

## Implementation roadmap (future; not AT0)

| Phase | Focus |
|---|---|
| **AT0** | This design ADR (done when accepted) |
| **AT1** | **Done** — ADR-127: single correction-field truth; Dev meters panel; louder import MissingSizingData |
| **AT2** | **Done** — ADR-128: runtime compose `definition baseline × instance` → presentation Transform; no collision change |
| **AT3** | **Done** — ADR-129: doodad collision/pick/occupancy use baseline×instance XZ; building footprint×instance everywhere; visual↔footprint validation |
| **AT4** | Production bake guarantees; CI/report gates; Excel column UX polish (meters-first) |
| **AT5** | Optional footprint helpers / unit collision validation; deprecate legacy scale authorities |

No new AI, no Excel edits, and no GLB re-exports are required to accept this design.

---

## Rejected designs

- Manually tuning every placed instance as the primary fix
- Runtime GLB bounds measurement in production every load
- Separate visual-scale and collision-scale authoring knobs
- Buildings with non-uniform scale
- Making ECS Transform authoritative
- Storing derived import AABBs on scene instances
- Silent clamp of extreme baselines to 1.0 without diagnostics
- Replacing ADR-097 wholesale instead of standardizing on it

---

## Consequences

- Artists and designers author **meters**; scale is derived
- Import owns measurement; catalogs own baked authority; instances own placement deltas
- Dev Mode explains size in meters
- Later AT phases implement; AT0 changes **no code, assets, or Excel**
