# Monster-pack GLB recovery status (updated after faithful batch)

Batch script: `run_faithful_batch_export.ps1`  
Results: `FAITHFUL_BATCH_RECOVERY.json` (29 inventory monsters; **28 re-exported**, **muscomorph** skipped as already complete, **bufomorph** skipped as reference).

## Outcome

All pack monsters with local source paths exported with **`manifest_complete: true`** and **`likely_child_translation_strip_export: false`** on the post-batch audit (faithful channel retention).

Excluded from batch (by design): `cavecrawler` (non-pack pipeline), `fox`, `human_*`, `robot`.

## Faithful references

| Asset | Notes |
|-------|--------|
| `bufomorph.glb` | First faithful reference + export manifest |
| `muscomorph.glb` | Representative validation before batch |

## Blockers

None on this machine while `inventory.json` source paths exist. Re-run batch on another machine only after refreshing `inventory.json` paths.
