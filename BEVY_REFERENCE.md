# BEVY_REFERENCE.md

# Purpose

This is a curated, project-scoped reference for the Bevy API changes that matter
to **chasma** (currently on `bevy = "0.18"`).

It is **not** a full changelog. It distills the parts of the official Bevy
migration guides that touch the systems this project uses now or in the near
roadmap phases: application/plugin composition, schedules and system sets,
reflection, resources, components, events/messages, observers, error handling,
windowing/`DefaultPlugins`, and math.

It covers the full jump from the last version this project's tooling knew well
(**0.16**) through **0.17** and **0.18**, because there is a 0.17 release in
between.

## How to use this file

- Treat it as a quick-reference for "what changed and what the current API is."
- Each entry tags the version the change landed in: `[0.17]` or `[0.18]`.
- For anything not covered here, or if something looks stale, consult the
  authoritative sources below. This file is a convenience, not a substitute.

## Sources (authoritative)

- 0.16 → 0.17 migration guide: https://bevyengine.org/learn/migration-guides/0-16-to-0-17/
- 0.17 → 0.18 migration guide: https://bevyengine.org/learn/migration-guides/0-17-to-0-18/

Always prefer these pages and the API docs at https://docs.rs/bevy/0.18 over
memory when writing code.

---

# Most relevant to Phase 0

These are the changes that directly affect `AppPlugin`, `WorldFoundationPlugin`,
`WorldConfig`, the coordinate types, and the runnable shell in `main.rs`
(see ADR-007).

## Reflection: types auto-register `[0.17]`

This directly affects ADR-007's decision to "register foundational types for
reflection in Phase 0."

- Types implementing `Reflect` are now **automatically registered** in the
  `TypeRegistry`. Most `app.register_type::<T>()` calls can be removed.
- Auto-registration is gated behind the `reflect_auto_register` feature, which
  **is part of Bevy's default features**. (Fallback: `reflect_auto_register_static`
  for platforms without `inventory` support, with project-structure caveats.)
- **Generic types are still NOT auto-registered** and must be registered
  manually with `app.register_type::<Foo<Bar>>()`.

Practical guidance for chasma:

- Derive `Reflect` on `ChunkCoord`, `LocalPosition`, `WorldPosition`,
  `WorldConfig`, `ChunkId`, etc. Non-generic types will register automatically
  with default features on.
- Only call `register_type` explicitly for generic instantiations, or if we
  ever disable default features.

## Reflect attribute syntax: parentheses only `[0.18]`

The `#[reflect(...)]` attribute now supports **only parentheses**, not braces or
brackets.

```rust
// 0.17 (any of these worked)
#[derive(Clone, Reflect)]
#[reflect[Clone]]

// 0.18 (parentheses only)
#[derive(Clone, Reflect)]
#[reflect(Clone)]
```

## `Resource` derive: no non-`'static` lifetimes `[0.18]`

A `#[derive(Resource)]` type that uses a non-`'static` lifetime will no longer
compile. Relevant to `WorldConfig` if it ever held borrowed data (it should own
its data anyway).

```rust
// Will NOT compile in 0.18
#[derive(Resource)]
struct Foo<'a> {
    bar: &'a str,
}
```

## System sets: `*Systems` naming convention `[0.17]`

Built-in system sets were renamed to consistently end in `Systems` (e.g.
`TransformSystem` → `TransformSystems`, `RenderSet` → `RenderSystems`,
`InputSystem` → `InputSystems`). When we define our own `SystemSet`s for layer
ordering in `AppPlugin`, follow the same `*Systems` convention.

## `Condition` → `SystemCondition` `[0.17]`

The run-condition trait `Condition` is now `SystemCondition`. Update imports if
we use run conditions.

## Schedule executors `[0.17] [0.18]`

- `SimpleExecutor` was deprecated in `[0.17]` and **removed in `[0.18]`**. Use
  `SingleThreadedExecutor` (small schedules) or `MultiThreadedExecutor` (large /
  async-heavy). The multithreaded executor is the default.
- Consequence: system ordering that used to be implicit from insertion order
  (the old `SimpleExecutor` behavior) must be made **explicit** via `.before()`,
  `.after()`, or `.chain()`. This matters for cross-layer ordering in `AppPlugin`.

## Error handling: always on, per-world `[0.17]`

- The `configurable_error_handler` feature is gone (remove it from features).
- There is no truly global handler. Use `App::set_error_handler(handler)`, or
  insert the `DefaultErrorHandler(handler)` resource for standalone worlds.

## `System::run` returns `Result` `[0.17]`

`System::run` and friends now return `Result`. Only relevant if we manually run
systems; `unwrap()` to match old behavior, or `?` in a `Result`-returning fn.

## Windowing / `DefaultPlugins` `[0.17] [0.18]`

Relevant because ADR-007 chose a runnable shell via `DefaultPlugins`.

- **`Window` split into multiple components `[0.17]`.** Some settings moved off
  `Window` onto sibling components on the same entity (e.g. cursor settings are
  now `CursorOptions`, and `WindowPlugin` takes `primary_cursor_options`).
- **`WindowResolution` constructed from `u32` `[0.17]`:**

```rust
// 0.16
WindowResolution::new(1920.0, 1080.0)
// 0.17+
WindowResolution::new(1920, 1080)
// from a UVec2
WindowResolution::from(some_uvec2)
```

- **`bevy_input` source features `[0.18]`.** If we ever build with
  `default-features = false`, input sources are now behind features. Enable the
  ones we use:

```toml
bevy = { version = "0.18", default-features = false, features = [
  "mouse", "keyboard", "gamepad", "touch", "gestures",
] }
```

(With default features on, this is a non-issue.)

## Math (`glam`) `[0.17]`

- `glam`, `rand`, and `getrandom` were updated. For our coordinate types this is
  mostly transparent: `IVec2`, `Vec3`, `UVec2`, etc. behave as before.
- `VectorSpace` gained an associated `Scalar` type (bounded by `ScalarField`);
  `f32`/`f64` are implemented out of the box. Only relevant if we manually
  implement `VectorSpace` (we shouldn't need to).

---

# Events vs Messages (terminology overhaul) `[0.17] [0.18]`

This is the single most confusing rename in the 0.16→0.18 span. We don't use
events in Phase 0, but we will later (streaming, lifecycle), so know this now.

- **"Buffered events" are now "messages."** `EventWriter`/`EventReader`/`Events<E>`
  → `MessageWriter`/`MessageReader`/`Messages<M>`. The trait is `Message` (derive
  `Message`).
- **"Event" / the `Event` trait now means observer events only.**
- Method renames `[0.17]`: `World::send_event` → `World::write_message`,
  `Commands::send_event` → `Commands::write_message`, `Events::send` →
  `Messages::write`, etc.
- A type can be both by deriving both `Message` and `Event`, but most types are
  one or the other.

## Observer API `[0.17] [0.18]`

- The observer parameter type `Trigger<E>` was renamed to **`On<E>`** `[0.17]`.
  Convention: name the variable after the event.

```rust
// 0.16
commands.add_observer(|trigger: Trigger<OnAdd, Player>| { /* trigger.target() */ });
// 0.17+
commands.add_observer(|add: On<Add, Player>| { /* add.entity */ });
```

- Lifecycle events lost their `On` prefix `[0.17]`: `OnAdd`/`OnInsert`/`OnReplace`/
  `OnRemove`/`OnDespawn` → `Add`/`Insert`/`Replace`/`Remove`/`Despawn`. (Watch for
  `Add` colliding with `std::ops::Add`.)
- **Targeted vs global events `[0.17]`.** `Event` is target-less (global) by
  default. Events that target an entity must derive `EntityEvent` and store the
  target entity on the struct. `world.trigger_targets(...)` was removed in favor
  of `world.trigger(Event { entity })`.
- **Entity events immutable by default `[0.18]`.** The mutable methods moved to a
  separate `SetEntityEventTarget` trait.

---

# Components / ECS details (near-term phases)

Relevant once Phase 1+ introduces chunk/terrain components and entities.

## Required components refactor `[0.17] [0.18]`

- Required-component priority now follows a consistent depth-first/preorder
  traversal of the dependency tree `[0.17]`.
- `Component::register_required_components` changed signature `[0.17]` (takes the
  current `ComponentId` plus a single `RequiredComponentsRegistrator`).
- `Bundle::register_required_components` was removed `[0.17]`.

## `RenderTarget` is a component `[0.18]`

When we eventually spawn cameras, `RenderTarget` is a separate required component
rather than a field on `Camera`:

```rust
// 0.18
commands.spawn((
    Camera3d::default(),
    RenderTarget::Image(image_handle.into()),
));
```

## State transitions always fire `[0.18]`

Setting the next state always triggers `OnEnter`/`OnExit`, even if the value is
unchanged. Use `next_state.set_if_neq(...)` for the old "only on change"
behavior. Relevant if we adopt Bevy states for app/world load phases.

## State-scoped entities renamed `[0.17]`

`StateScoped` → `DespawnOnExit`; new `DespawnOnEnter`. `add_state_scoped_event`
is replaced by `add_event` + `clear_events_on_exit` / `clear_events_on_enter`.

---

# Rendering / assets (later phases — low priority now)

Per ADR-004 we avoid renderer complexity early, so these are background context
for Phase 2+ only.

## `bevy_render` crate reorganization `[0.17]`

Many types moved out of `bevy_render` into focused crates. Import from `bevy::*`
prelude paths or the new crates:

- Cameras/visibility/culling (`Camera`, `Camera3d`, `Visibility`, `Aabb`,
  `Frustum`, etc.) → `bevy_camera` (`bevy::camera`).
- Shaders (`Shader`, `ShaderRef`, etc.) → `bevy_shader` (`bevy::shader`).
- Lights (`PointLight`, `DirectionalLight`, `AmbientLight`, etc.) → `bevy_light`
  (`bevy::light`).
- Meshes (`Mesh`, `Mesh3d`, `Mesh2d`, `Indices`, `Meshable`) → `bevy_mesh`
  (`bevy::mesh`).
- Images (`Image`, `ImagePlugin`, `ImageFormat`, samplers) → `bevy_image`
  (`bevy::image`).
- `RenderAssetUsages` now comes from `bevy_asset` (`bevy::asset`), not
  `bevy_render`.

## `Mesh` mutation can fail `[0.18]`

`Assets<Mesh>` now retains `RENDER_WORLD`-only meshes even after extraction.
Mutating methods gained `try_*` variants returning `Result<_, MeshAccessError>`
(e.g. `try_insert_attribute`, `try_compute_normals`, `try_with_inserted_indices`).
The non-`try_` versions panic if the mesh was already extracted. Relevant to
Phase 2 terrain mesh generation.

## Automatic `Aabb` for meshes/sprites `[0.18]`

Bevy creates and now also **updates** an `Aabb` for mesh/sprite entities (used
for visibility and picking). The old workaround of removing `Aabb` after mesh
edits is no longer needed. Use the `NoAutoAabb` component to opt out.

## `AmbientLight` split `[0.18]`

`AmbientLight` is now a per-camera component; the world default is the
`GlobalAmbientLight` resource (added by `LightPlugin`). Rename resource usage to
`GlobalAmbientLight`.

## `MeshletMesh` assets must be regenerated `[0.18]`

Virtual geometry's asset format changed. (Not used by chasma yet.)

## Cargo feature collections `[0.18]`

Bevy added high-level feature collections (`2d`, `3d`, `ui`) and mid-level ones
(`2d_api`, `3d_api`, `default_app`, `default_platform`). If we ever trim default
features, prefer these collections over hand-listing individual features.

---

# Maintenance

When upgrading Bevy or when an API here looks wrong:

1. Open the relevant migration guide(s) listed under Sources.
2. Update the affected entries and bump the version tags.
3. Keep this file scoped to what chasma actually uses — do not mirror the full
   changelog.
