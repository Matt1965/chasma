# ADR-052: Time of Day Visual Environment System (E10)

# Status

Accepted (E10 — visual day-night cycle foundation)

# Context

ADR-026 established the Environment rendering layer with [`EnvironmentSettings`] as the
single tuning authority for skybox, ambient fill, and directional light. The dev preview
needs a prototype day-night cycle for visual polish without introducing gameplay simulation
time, weather, or calendar systems.

# Decision

## Visual-only scope

Time of day is **client-local presentation**. It does not live in [`WorldData`], does not
advance simulation ticks, and does not affect units, terrain, doodads, or pathfinding.

## Environment ownership

New modules under `src/environment/`:

| Module | Role |
|--------|------|
| `time_of_day.rs` | [`TimeOfDaySettings`] resource — clock, pause, cycle length, twilight tuning |
| `cycle.rs` | Lighting evaluation, settings sync, ECS presentation sync, dev keyboard |

The cycle writes into [`EnvironmentSettings`]; [`sync_environment_presentation`] applies
settings to the existing singleton directional light, [`GlobalAmbientLight`], and
[`Skybox`] brightness. No duplicate lights are spawned.

## TimeOfDaySettings

- `enabled`, `paused`, `time_hours` (0–24), `day_length_seconds`
- `sunrise_hour`, `sunset_hour`, `sun_pitch_min_deg`, `sun_pitch_max_deg`
- `night_ambient_multiplier`

When `enabled == false`, the cycle does not mutate [`EnvironmentSettings`].

## Lighting model (prototype)

- Solar noon → peak directional illuminance, ambient, skybox brightness
- Night → low illuminance and ambient scaled by `night_ambient_multiplier`
- Sunrise/sunset → warmer directional color via twilight warmth
- Sun rotation derived from clock hour and daylight elevation arc

## Dev controls (feature `dev`)

Keyboard when F12 dev mode is active:

| Key | Action |
|-----|--------|
| T | Toggle cycle enabled |
| P | Toggle pause |
| `[` / `]` | Step time ±1 hour |
| `,` / `.` | Decrease / increase day length |
| `6` | Jump to sunrise |
| `1` | Jump to noon |
| `0` | Jump to midnight |

# Future hooks

- **Weather** — modulate ambient/skybox/directional on top of time-of-day baseline
- **Water** — reflect sky tint from evaluated lighting
- **Gameplay time** — separate simulation clock may *read* visual time for UI only, or
  drive Environment settings from authoritative sim time later; not coupled in E10
- **Save/load** — persist visual time in client settings or scene metadata, not WorldData

# Non-goals (E10)

No weather, water, moon/stars, gameplay schedules, crops, calendar, or WorldData persistence.

# References

- ADR-026 Environment Rendering Layer
- E10 implementation: `src/environment/time_of_day.rs`, `src/environment/cycle.rs`
