Doodad definition authoring (R6)
=================================

Place the Excel workbook here for dev import:

  Doodads.xlsx   (worksheet name: Doodads)

When running with `--features dev`, startup attempts:

  source_data/doodads/Doodads.xlsx
    -> validate rows
    -> build DoodadCatalog
    -> optional export to assets/doodads/catalog.ron

If the workbook is missing or invalid, the engine falls back to the in-code
starter catalog and continues startup.

Column headers (exact names; order does not matter):

  Name, Description, Category, Biome, File Path,
  Min Size, Max Size, Spawn Weight, Random Rotation (Y/N), Enabled

See `Chasma Design` documentation for authoring rules.
