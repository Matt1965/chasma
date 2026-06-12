# ADR-014: Camera Layer Boundaries

# Status

Accepted

# Context

Chasma needs a permanent RTS-style camera (Kenshi-like: orbit a focus point, pan on
the horizontal plane, rotate with mouse drag, zoom by distance). Phase 2A's dev
preview spawned a fixed debug `Camera3d` inside `TerrainPreviewPlugin`. That is
composition throwaway, not the long-term camera architecture (ADR-007, ADR-010).

ARCHITECTURE Principle 3 states rendering owns no authoritative game state.
Principle 1 states simulation is the authority and rendering is a representation.
The Rendering Layer is "visual representation only." Camera control is client-local
view presentation: it must not become a dependency of world data, terrain, or
future simulation.

Multiplayer is not implemented, but ARCHITECTURE requires architecture remain
compatible: camera pose must be local to each client and must not be assumed
authoritative or replicated by default.

# Decision

## A distinct Camera layer

Introduce a Camera layer at `src/camera/`, registered by `CameraPlugin` in
`AppPlugin` after foundational layers (ADR-007). It is separate from the Terrain
Runtime Layer (`src/terrain/`) and the World Data Layer (`src/world/`).

## Ownership split

- **Camera layer owns client-local view presentation only:** orbit pose
  (focus, yaw, pitch, distance), input response, smoothing, and the main
  `Camera3d` entity marker (`RtsCamera`).
- **Camera layer does not own:** terrain data, world geography, simulation state,
  selection, UI policy, or networked authority.
- **World / terrain / simulation never depend on camera.** The camera layer must
  not be imported by `src/world/`, `src/terrain/`, or future simulation code.
- **Camera foundation has zero dependency on `crate::world` or `crate::terrain`.**
  Initial pose defaults live in `CameraSettings`, not in world queries.

## Control model (foundation)

Focus-point orbit RTS camera:

- `focus` — look-at point; WASD pans it on the XZ plane relative to camera yaw
- `yaw` — full 360° rotation around world Y
- `pitch` — clamped elevation angle; camera always looks at `focus`
- `distance` — orbit radius; mouse wheel zoom within configured min/max
- Middle-mouse drag adjusts yaw and pitch
- Transform is derived entirely from focus/yaw/pitch/distance each frame

## Multiplayer stance

Camera state is stored on local client ECS components/resources. It is not written
to `WorldData`, is not part of the authoritative world model, and is not
replicated unless a future ADR explicitly defines networked camera behavior.

## Dev preview integration

`TerrainPreviewPlugin` retains terrain load/render and lighting only. The
permanent camera comes from `CameraPlugin`. Preview must not spawn a competing
camera entity.

## Deferred (not in foundation)

Terrain following/height grounding, camera collision, underground prevention,
unit follow, selection framing, edge scrolling, UI pointer blocking, input
rebinding, multiple camera modes, gameplay action mapping, network
synchronization.

# Rationale

Separating camera from terrain preview prevents presentation concerns from leaking
into the terrain runtime vertical slice (ADR-010). Keeping the layer free of world
imports preserves one-way data flow and future headless/server builds (ADR-007).
Local-only camera state is the minimal multiplayer-compatible default.

# Consequences

Benefits:

- Permanent RTS camera architecture with clear boundaries
- World/terrain/simulation remain camera-agnostic
- Dev preview uses the same camera players will use

Costs:

- Initial camera pose is configured in `CameraSettings`, not derived from loaded
  terrain (until a future optional seam exists)
- `AppPlugin` gains another layer registration

# Alternatives Considered

## Camera inside terrain preview or rendering stub

Rejected: couples view control to terrain dev wiring; violates layer boundaries.

## Camera reads WorldData for initial focus

Rejected for foundation: creates world → camera dependency inversion risk and
ties presentation to authoritative store.

## Network-replicated camera from the start

Rejected: no multiplayer consumer; violates Groundwork Rule (AGENTS.md).

# Notes

Cross-references ADR-007 (composition), ADR-010 (terrain does not own camera).
BEVY_REFERENCE: use `AccumulatedMouseMotion` / `AccumulatedMouseScroll` and
`ButtonInput` for Bevy 0.18 input.
