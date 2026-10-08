# Monster-pack GLB recovery status

Heuristic: `likely_child_translation_strip_export` — `IdleBreathe` has **zero** child-bone translation channels (typical of old `sanitize_file` default on export). Full audit: `AFFECTED_GLBS.json`.

## Faithful (reference)

| Asset | Status |
|-------|--------|
| `bufomorph.glb` | Regenerated on faithful path; 21 clips; rich idle channels |

## Likely legacy strip export (re-export with faithful `monster_export_core`)

All listed untracked pack GLBs under `assets/units/` except bufomorph, fox, humans, robot, cavecrawler (different pipeline). Representative re-export before batch: **muscomorph** (pending — run `blender_export_monster.py` with inventory `source_dir`).

| render_key | anims | idle child trans ch |
|------------|------:|--------------------:|
| arthromahre | 21 | 0 |
| carcidonte | 36 | 0 |
| clypeosaurus | 26 | 0 |
| deinodonte | 21 | 0 |
| densoptere | 27 | 0 |
| drackmahre | 32 | 0 |
| entomorane | 29 | 0 |
| giant_slug | 28 | 0 |
| gorosaurus | 41 | 0 |
| gryllunguis | 15 | 0 |
| hellcreeper | 25 | 0 |
| hideoplast | 30 | 0 |
| karcinomorph | 29 | 0 |
| karckmahre | 24 | 0 |
| laminferox | 34 | 1 |
| letalobrach | 31 | 0 |
| marhomorph | 31 | 0 |
| morphorrid | 38 | 0 |
| muscomorph | 26 | 0 |
| pardathrox | 27 | 0 |
| perderos | 41 | 0 |
| scolomorph | 37 | 0 |
| scyver | 37 | 0 |
| skorpmare | 18 | 0 |
| telluropod | 16 | 0 |
| tetrachnide | 29 | 0 |
| tyranopode | 31 | 0 |
| vespomorph | 25 | 0 |

## Excluded / special

| Asset | Note |
|-------|------|
| `cavecrawler.glb` | Non–monster-pack converter; stripping documented separately |
| `fox`, `human_*`, `robot` | Not pack batch |

## Blockers

Re-export requires pack paths from `inventory.json` on the authoring machine (`C:\Users\matt1\My project\Assets\...`). Missing paths must be flagged per monster before batch regen.
