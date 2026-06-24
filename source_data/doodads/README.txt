Doodad definition authoring (R6)
=================================

Doodad rows live on the **Doodads** worksheet in the repo-root design workbook:

  Chasma Design.xlsx

When running with `--features dev`, startup imports that workbook (Units + Doodads sheets)
via the crate manifest path, builds runtime catalogs, and optionally exports doodads to
`assets/doodads/catalog.ron`.

Column headers (exact names; order does not matter):

  Name, Description, Category, Biome, File Path,
  Min Size, Max Size, Spawn Weight, Random Rotation (Y/N), Enabled

See `Chasma Design` documentation for authoring rules.
