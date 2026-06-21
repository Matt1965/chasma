# Environment presentation assets (R8 / ADR-026)

Permanent home for client-local environment rendering assets.

```text
assets/environment/
    skyboxes/
        {set_name}/
            cubemap.ktx2
            cubemap.png
```

See `skyboxes/default/README.txt` for the default skybox drop-in layout.

Future environment assets (HDR probes, atmosphere textures, etc.) should live
under `assets/environment/` in dedicated subfolders — not under terrain or doodad paths.
