# ADR-013: Terrain Mesh Generation and LOD

# Status

Accepted

# Context

The Terrain Runtime Layer (ADR-010) turns authoritative heightfield tiles into
derived, disposable meshes for rendering. ROADMAP Phase 2 lists both "terrain mesh
generation" and a "terrain LOD system" / "basic far terrain representation," with
success criteria "terrain is visible," "LOD works," and "high-altitude viewing is
functional."

ADR-004 forbids introducing renderer complexity (custom shaders, pipelines,
GPU-driven systems) without profiling evidence. These two forces must be
reconciled: LOD is in scope, but only as far as generated meshes + standard
materials allow.

# Decision

## Pure mesh builder

Mesh generation is a **pure function** of an authoritative `Heightfield` (plus a
LOD level): it takes the tile and returns a Bevy `Mesh`, with no access to the
ECS world. This makes it unit-testable on synthetic data and keeps it free of
rendering/world side effects (ADR-010).

- Positions come from the `(N+1) x (N+1)` grid; an `N x N` quad grid yields
  `2 * N * N` triangles.
- Per-vertex normals are computed from heightfield finite differences.
- Use Bevy 0.18 mesh APIs: the `try_*` mutation methods
  (`try_with_inserted_indices`, `try_compute_normals`, etc.) per BEVY_REFERENCE,
  and rely on automatic `Aabb`.
- The mesh is positioned by a `Transform` at the chunk's minimum-corner origin
  (`cx * size, 0, cz * size`; ADR-001). Heights are absolute world Y.

The builder consumes authoritative data and writes nothing back; meshes are
disposable and fully regenerable.

## Materials

A single shared `StandardMaterial` is used initially. No custom terrain material,
shader, or pipeline is introduced (ADR-004). Material blending from masks is a
later concern (masks are deferred, ADR-009).

## Phase 2A: single full-resolution LOD

Phase 2A generates exactly **one full-resolution mesh per chunk**:

- No LOD levels, no LOD selection, no skirts (with a single LOD there are no
  inter-LOD cracks to hide).
- This proves the pure data → mesh → render-entity path correctly and is the
  mesh side of the Phase 2A vertical slice.

## LOD: designed, deferred

The LOD system is designed now and implemented in a later slice (Phase 2C):

- **Mesh-resolution LOD only**: discrete levels produced by subsampling the
  heightfield at strides (1, 2, 4, ...), giving lower-triangle tiles for distant
  chunks. This stays within ADR-004 (generated meshes, standard material).
- Distance-based selection picks a level per chunk from camera distance.
- **Cracks between adjacent LODs are hidden with skirts** (vertical edge geometry)
  as the simple, robust option.
- A **basic far-terrain representation** (coarsest LOD, or a simple coarse
  aggregate) covers high-altitude viewing.

Deferred beyond Phase 2 (require profiling per ADR-004): continuous/geomorphing
LOD, LOD edge stitching beyond skirts, GPU tessellation, clipmaps, impostors,
virtual texturing, custom render phases.

## Known seam-normal nuance

Shared edge **positions** are continuous across chunks (ADR-008), so meshes meet
without gaps. **Normals** at a tile's border, computed only from in-tile samples,
are slightly discontinuous between neighbors. Phase 2 accepts this with the
standard material; computing border normals from neighboring resident tiles is a
documented follow-up, not Phase 2A scope.

# Rationale

A pure builder is the cleanest way to keep mesh generation derived and testable.
Restricting LOD to mesh-resolution subsampling satisfies the ROADMAP LOD criteria
without violating ADR-004. Deferring LOD out of Phase 2A keeps the first slice
focused on proving the architecture, not visual breadth.

# Consequences

Benefits:

- Testable, side-effect-free mesh generation on synthetic heightfields.
- Phase 2A has no crack/skirt complexity (single LOD).
- LOD path is designed so Phase 2C extends the same builder (it already takes a LOD
  level).

Costs:

- Single full-resolution meshes are not viable for large/distant worlds; LOD is
  required before high-altitude viewing is efficient (hence Phase 2C).
- Border-normal discontinuity is visible under some lighting until the follow-up
  lands.

# Alternatives Considered

## Custom terrain shader/material in Phase 2

Rejected: violates ADR-004 without profiling evidence; standard material suffices
for the Phase 2 criteria.

## Multi-LOD and skirts in Phase 2A

Rejected for 2A: LOD is breadth; including it would obscure whether the core
data → mesh → render path is correct and add crack-handling before it is needed.

## Continuous LOD / geomorphing

Rejected for Phase 2: renderer complexity without evidence (ADR-004).

# Notes

The builder's signature takes a LOD level from the start so adding levels in
Phase 2C is additive. Skirts and far-terrain are sketched here only to ensure the
builder and asset format do not preclude them.
