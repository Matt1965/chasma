# ADR-005: Initial World Query API

# Status

Accepted

# Context

Future gameplay systems should not depend directly on terrain meshes, render entities, or internal chunk structures.

The project needs a small initial query API that supports the immediate runtime without creating speculative bloat.

Several possible queries were considered and rejected because they lacked clear early consumers.

# Decision

The initial public query API should remain small.

The first accepted query concepts are:

- chunk_at(position)
- height_at(position)
- slope_at(position)
- normal_at(position)
- doodads_near(position, radius)
- is_chunk_loaded(chunk)

Deferred future queries include:

- is_blocked(position)
- can_place_footprint(footprint)

Deleted or internal-only query concepts include:

- chunk_bounds(chunk)
- terrain_exists_at(position)
- biome_at(position)
- mask_value_at(...)
- placement_density_at(...)
- is_inside_doodad_exclusion(position)
- chunk_lod(chunk)
- site_at(position)
- nearest_site(...)

# Rationale

Each accepted query has a clear consumer.

## chunk_at(position)

Used for:

- streaming
- saving
- loading
- placement
- chunk ownership

## height_at(position)

Used for:

- terrain placement
- doodad placement
- future movement
- future projectiles
- future pathfinding

## slope_at(position)

Used for:

- doodad placement
- future roads
- future pathfinding
- future building restrictions

## normal_at(position)

Used for:

- terrain alignment
- building orientation
- doodad orientation
- slope-facing calculations

## doodads_near(position, radius)

Used for:

- authored/procedural object lookup
- future interaction
- future harvesting
- future gameplay queries

## is_chunk_loaded(chunk)

Used for:

- streaming
- debugging
- safe runtime checks

# Consequences

Benefits:

- Avoids speculative API bloat
- Keeps gameplay decoupled from rendering
- Supports immediate runtime needs
- Leaves room for future systems

Costs:

- Some future systems will require API expansion
- Occupancy and placement queries are deferred

# Alternatives Considered

## Large query API from the beginning

Rejected because many proposed queries did not have clear current consumers.

## Exposing internal terrain masks publicly

Rejected because masks are primarily terrain/doodad generation inputs, not general gameplay queries yet.

# Notes

Query APIs should grow only when there is a clear consumer.

Gameplay should ask the world questions.

Gameplay should not inspect rendered terrain meshes or render entities for truth.