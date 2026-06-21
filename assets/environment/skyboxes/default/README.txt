# Default skybox (`assets/environment/skyboxes/default/`)

Drop-in cubemap assets for the environment skybox (R8 / ADR-026).

## Option A — loose face PNGs (authoring-friendly)

Place six **square** PNG faces in this folder:

| File | Cubemap axis |
|------|----------------|
| `right.png` | +X |
| `left.png` | −X |
| `top.png` | +Y |
| `bottom.png` | −Y |
| `front.png` | +Z |
| `back.png` | −Z |

Merge into the runtime format (from project root):

```text
cargo run --bin merge_skybox_cubemap -- default
```

This writes `cubemap.png` (six faces stacked vertically) for the Bevy skybox loader.

## Option B — single cubemap file

| File | Format |
|------|--------|
| `cubemap.ktx2` | **Preferred.** KTX2 cubemap with cubemap metadata. |
| `cubemap.png` | Six square faces stacked vertically (height = 6 × width). |

If both `cubemap.ktx2` and `cubemap.png` exist, `ktx2` is chosen first.

## Replacing the skybox

Update face PNGs and re-run the merge command, or replace `cubemap.ktx2` / `cubemap.png` directly.
Future sets live as sibling folders under `assets/environment/skyboxes/` (e.g. `night_clear/`).

## Missing asset behavior

If no cubemap file is present, the engine logs a warning and continues without a skybox.
