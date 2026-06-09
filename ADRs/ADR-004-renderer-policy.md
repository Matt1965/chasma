
---

## `ADRs/ADR-004-renderer-policy.md`

```markdown
# ADR-004: Renderer Complexity Policy

# Status

Accepted

# Context

The project targets:

- large terrain
- long view distance
- chunk streaming
- many doodads
- future simulation systems

It is tempting to introduce custom shaders, GPU-driven rendering, custom render phases, or custom instancing early.

The previous project experienced confusion and complexity when moving too quickly into custom rendering work.

# Decision

Do not introduce custom renderer complexity unless profiling demonstrates a need.

Avoid introducing the following as foundational systems:

- custom shaders
- custom terrain materials
- custom render pipelines
- custom render phases
- GPU-driven vegetation
- custom instancing systems

Use existing Bevy rendering capabilities first.

Renderer-specific systems must remain replaceable and must not own authoritative world state.

# Rationale

The immediate risk is poor data ownership, not insufficient shader complexity.

Most early project goals can be achieved with:

- generated meshes
- standard materials
- Bevy's normal rendering path
- chunk-based streaming
- LOD meshes
- built-in instancing where possible

Custom rendering should be an optimization path, not an architectural foundation.

# Consequences

Benefits:

- Less renderer complexity
- Less WGSL/custom pipeline work
- Cleaner architecture
- Easier iteration
- Easier future renderer replacement

Costs:

- Initial visuals may be simpler
- Terrain material blending may be less advanced early
- Dense vegetation may hit limits sooner
- Some advanced visual effects are deferred

# Alternatives Considered

## Custom terrain renderer from the beginning

Rejected because it increases complexity before proving need.

## GPU-driven vegetation from the beginning

Rejected because it is not required for the initial runtime foundation.

# Notes

This decision does not ban custom rendering forever.

It requires evidence before introducing it.