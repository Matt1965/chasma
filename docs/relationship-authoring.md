# Relationship Authoring

> **Status: Phase 1–7 implemented (locked Relationship Map complete).** Incidents, reputation
> propagation, personality, and full creature behavior remain deferred.

## Authoritative workbook

Relationship authoring uses the repository-root workbook:

`Chasma Design.xlsx`

Planned flow, matching the existing catalog pipeline:

1. Author identity namespaces (**Factions**, **Species**) and one or more relationship matrix sheets.
2. Dev startup imports and validates identity namespaces first, then matrices.
3. Matrices normalize into sparse directed edges in the authored Disposition layer.
4. A human-readable import report is written to `logs/relationship_matrix_report.md`.

Production builds do not read Excel (see [docs/item-authoring.md](item-authoring.md) for the same
pattern).

## Identity namespaces

Relationship identity keys are **stable readable semantic slugs**, lowercase `snake_case`. They are
what appears in matrix headers, so legibility is a hard requirement.

### Factions

| Column | Required | Notes |
|--------|----------|-------|
| Faction Key | yes | Stable relationship identity, e.g. `player`, `wild`, `trinity` |
| Name | yes | Display name — presentation resolves this from the catalog |
| Faction ID | no | Legacy `F-000X` authoring cross-reference only; **never** a matrix header |
| Enabled | yes | `Y` / `N` |
| Description | no | |

The existing one-dimensional `Disposition` column is **retired**. A directional
`Faction -> Faction` matrix replaces it. Two relationship authorities must not coexist.

### Species

Keep this sheet minimal. Do not add speculative species mechanics.

| Column | Required | Notes |
|--------|----------|-------|
| Species Key | yes | Stable relationship identity, e.g. `human`, `yaratan`, `cavecrawler` |
| Name | yes | Display name |
| Enabled | yes | `Y` / `N` |
| Description | no | |

"Race" and "Species" mean the same thing in Chasma — use **Species**.

Species is deliberately **not** the unit definition id: multiple unit definitions must be able to
share a species, which is what makes cross-domain Species relationships authorable.

### Unit identity

Units reference identity namespaces by key. A unit carries no independent display name for its
faction; presentation resolves that from the Factions catalog.

The existing `Faction` column on the **Units** sheet was migrated to `Faction Key` + `Species Key`
in Phase 1.

## Relationship matrices

### Layout

Rows are the **source / observer**. Columns are the **target**. Each cell is the row's relationship
toward the column. Matrices are **asymmetric by design**.

```text
        A                    B        C        D
1   Faction -> Species      bug      human    yaratan
2   trinity                 -300     0        0
3   wild                    -50      -100     -100
4   player                  0        300      0
```

`A1` declares the domain pair. It is simultaneously the human-readable corner label of the matrix
and the machine-readable domain declaration — which is exactly what a matrix corner cell means:
*rows are this, columns are that*. This is preferred over encoding domains in the sheet name, which
breaks silently when a tab is renamed.

Sheets are discovered by name prefix (`Rel `), so **adding a new matrix requires no code change**.

### Reading the example

```text
Faction trinity -> Species bug     = -300
Species bug -> Faction trinity     =    ?   (authored separately, in a Species -> Faction matrix)
```

Direction is never inferred and never mirrored.

### Supported domain pairs

Any combination of the initial domains, including cross-domain:

```text
Faction -> Faction        Species -> Species
Faction -> Species        Species -> Faction
```

Same-domain square matrices and cross-domain rectangular matrices share **one** importer. A square
matrix is simply the case where the source and target domains are equal; it is not special-cased.

Only combinations actually needed require a sheet. Absent combinations are not an error.

**Initial gameplay edge (Phase 6):** `Rel Faction Faction` cell `wild -> player = -300`. Reverse
`player -> wild` remains absent (`0`). This asymmetry is intentional: wild units may hate player
units without player units automatically hating wild units.

### Cells

- Signed integers. Negative is worse, positive is better.
- **Blank or `0` stores no contribution.** A missing edge contributes 0.
- No clamp. Values outside the ordinary range are legal and meaningful — a creature may sit at
  roughly −300 specifically so that small positive modifiers do not quickly overcome a deeply
  negative baseline.
- All applicable contributions **stack additively** across every matrix. There is no precedence and
  no "most specific wins".

Worked example — three matrices contributing to one resolved pair:

```text
Faction trinity   -> Species bug     -300     (Faction -> Species matrix)
Species human     -> Species bug     -100     (Species -> Species matrix)
Individual Unit A -> Individual Unit B  +150  (mutable Standing, not authored)
-------------------------------------------------
Effective                            -250
```

### Validation

| Situation | Behavior |
|-----------|----------|
| Unknown domain in `A1` | Hard error, sheet aborted |
| Duplicate id in row or column headers | Hard error, sheet aborted |
| Same directed edge defined in two sheets | **Hard error** — additive stacking would silently double it |
| Unknown id in a header | Row/column failure + warning; entry skipped |
| Non-numeric cell | Row failure + warning |
| Blank / `0` cell | No stored contribution |

Duplicate-edge detection is a correctness requirement, not a nicety: because contributions stack,
a duplicated edge does not override — it doubles, silently.

## Avoiding transposed matrices

A transposed matrix is the primary authoring hazard, and for **square** matrices it is undetectable
by validation — a transposed `Species -> Species` grid is still structurally valid. Three defenses:

1. The `A1` corner declaration keeps direction visible while authoring.
2. The import report echoes edges back in prose direction form, so a transpose is visible on
   inspection.
3. Author self-relationships on the diagonal, giving a familiar visual anchor in square matrices.

If a matrix comes out perfectly symmetric, treat that as suspicious — perfect symmetry in a
directional matrix is more often a copy-paste artifact than a design intent.

## Import report

`logs/relationship_matrix_report.md` (regenerated each dev startup, gitignored like other reports)
records:

- every authored edge in prose direction form — `Faction "trinity" -> Species "bug" = -300`
- unknown ids, duplicate headers, duplicate directed edges
- per-sheet counts, including how many cells were blank or zero

Precedent: `logs/asset_sizing_report.md`, `logs/navigation_blueprint_report.md`.

## What authored relationships do *not* control

Authored values are **disposition**, not behavior. They do not decide whether a unit attacks.

- Behavior and AI interpret relationship; a threshold used by the placeholder combat AI is tuning,
  not architecture.
- The player may deliberately attack any mechanically valid target regardless of relationship.
  Relationship is not an invisible protection mechanism.
- Mutable reputation (the Standing layer) is not authored here — it accumulates at runtime.
- Ownership, `TeamId`, and `Affiliation` are unrelated concepts and are not authored through
  relationship matrices.

See [ADR-132](../ADRs/ADR-132-relationship-and-reputation-architecture.md) §8.

## Dev diagnostics

### Relationship Links overlay

The Debug window **Relationship Links** toggle is a dev-only presentation consumer. It:

- reads mutual perception from the Phase 4 perception authority (bounded spatial queries, not a separate sight system)
- resolves directional totals through `effective_relationship_for_records` only (no authored-matrix or Standing reads in overlay code)
- does not own or mutate relationship, perception, or combat state

Links render only for mutually perceiving alive units within a dev-only visualization distance cap (32 m; not perception authority). Toggle state lives on `DevModeState.debug_config` / `DebugOverlayConfig` and persists while the Debug window is closed.

### Combat trace log

High-volume combat, perception, and autonomous-desire diagnostics append to `logs/combat.log` (fresh session header each run). Routine `COMBAT_TRACE` / `PERCEPTION_TRACE` lines no longer flood the terminal during `cargo run --features dev`. Import reports such as `logs/relationship_matrix_report.md` stay in their existing destinations.
