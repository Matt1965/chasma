# ADR-099: Dev Transform Gizmos and Edit Transactions (DT3)

# Status

Accepted (DT3)

# Context

DT1 established metric sizing and authoritative transform types. DT2 added doodad numeric
editing, ellipse occupancy, picking, and `update_doodad_transform`. DT3 adds Blender-style
on-screen translate, rotate, and scale gizmos with drag preview and authoritative commit.

# Decision

## Custom gizmo system (`dev/gizmo/`)

No external editor crate. Uses Bevy `Gizmos` for drawing and analytic ray tests for handle
picking. Client-local preview; authoritative commit on mouse release via DT2 APIs.

## Tool state

`DevToolState` tracks `DevTool`: Select, Place, Translate, Rotate, Scale.

- Entering transform mode clears placement preview.
- Arming placement sets Place and exits transform drag.
- Disabling Dev Mode clears gizmo state.

## Transform edit state

`TransformEditState` (client-local) holds target (`SelectedWorldObject`), mode, coordinate
space (World/Local), active handle, drag rays, start/preview placement, snap settings, and
validation status. No authoritative data stored only here.

## Capability filtering

Uses `TransformCapabilities` from DT1:

- **Doodad:** full XYZ translate/rotate/scale + plane handles + uniform scale center.
- **Building (preview only, DT4 commit):** capability-aware handles drawn; commit blocked.
- **Unit:** no gizmo.

Scale handles always use **local axes** (documented in inspector). World/Local toggles
translation and rotation only.

## Pick priority

1. UI panels
2. Gizmo handles (when transform tool active)
3. Placement tool
4. World-object selection
5. Gameplay commands

During drag: camera, selection, placement, and gameplay mouse blocked via `DevModeInputGate`.

## Preview / commit

- Drag: `WorldData` unchanged; `DevTransformPreview` component overrides render `Transform`
  after doodad sync.
- Collision preview: computed and drawn; occupancy not registered until commit.
- Release: `update_doodad_transform` with quantized candidate.
- Failure: restore preview to drag-start; show error in inspector.
- Esc: cancel drag; restore authoritative presentation.

## Apparent screen size

Gizmo scale derived from camera distance, viewport height, and fixed FOV heuristic — clamped.
Does not affect authoritative transforms.

## Keyboard shortcuts (transform context)

| Key | Action |
|-----|--------|
| W | Translate mode |
| E | Rotate mode |
| R | Scale mode (not building ruins) |
| L | Toggle World/Local |
| X/Y/Z | Axis constraint during drag |
| Esc | Cancel drag / exit transform mode |

`E` catalog filter suppressed when doodad/building selected for gizmo context.

## Building DT4 seam

Buildings may display capability-filtered gizmo handles. `policy_for_target` returns
`can_commit: false` with DT4 diagnostic until authoritative building transform API exists.

# Consequences

- Responsive visual editing without ECS-as-truth.
- DT2 transaction API remains single commit path.
- DT4 can enable building commit without gizmo rewrite.

# Related

- ADR-098 — doodad transform editing (DT2)
- ADR-097 — authoring transform types (DT1)
- ADR-044 — dev placement preview (gizmos-only pattern)
