use std::collections::HashMap;
use std::path::Path;

use crate::data_import::error::DataImportError;
use crate::world::relationship::authored::{
    AuthoredFacetKey, AuthoredRelationshipCatalog, DirectedRelationshipEdgeKey,
};
use crate::world::{FactionCatalog, SpeciesCatalog};

use super::matrix_domain::MatrixDirection;
use super::matrix_excel::{
    RELATIONSHIP_MATRIX_SHEET_PREFIX, cell_to_string, discover_relationship_matrix_sheet_names,
    range_cell, read_relationship_matrix_range,
};
use super::normalize_relationship_key;

pub const RELATIONSHIP_MATRIX_REPORT_PATH: &str = "logs/relationship_matrix_report.md";

/// One imported authored edge recorded for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedRelationshipEdge {
    pub sheet: String,
    pub source: AuthoredFacetKey,
    pub target: AuthoredFacetKey,
    pub value: i32,
}

/// Per-sheet matrix import outcome.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RelationshipMatrixSheetSummary {
    pub sheet: String,
    pub direction: Option<MatrixDirection>,
    pub imported_edges: usize,
    pub blank_or_zero_cells: usize,
    pub skipped_rows: usize,
    pub skipped_columns: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub aborted: bool,
}

/// Workbook-wide relationship matrix import outcome.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RelationshipMatrixImportSummary {
    pub sheets_discovered: usize,
    pub sheets_imported: usize,
    pub sheets_aborted: usize,
    pub edges_imported: usize,
    pub blank_or_zero_cells: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub sheet_summaries: Vec<RelationshipMatrixSheetSummary>,
    pub imported_edges: Vec<ImportedRelationshipEdge>,
}

pub fn import_authored_relationship_matrices_from_excel(
    path: &Path,
    factions: &FactionCatalog,
    species: &SpeciesCatalog,
) -> Result<(AuthoredRelationshipCatalog, RelationshipMatrixImportSummary), DataImportError> {
    let sheet_names = discover_relationship_matrix_sheet_names(path)?;
    let mut summary = RelationshipMatrixImportSummary {
        sheets_discovered: sheet_names.len(),
        ..RelationshipMatrixImportSummary::default()
    };

    if sheet_names.is_empty() {
        summary
            .warnings
            .push("no relationship matrix sheets discovered (`Rel ` prefix)".to_string());
        return Ok((AuthoredRelationshipCatalog::default(), summary));
    }

    let mut edge_sources: HashMap<DirectedRelationshipEdgeKey, (i32, String)> = HashMap::new();

    for sheet in sheet_names {
        let range = read_relationship_matrix_range(path, &sheet)?;
        let mut sheet_summary = RelationshipMatrixSheetSummary {
            sheet: sheet.clone(),
            ..RelationshipMatrixSheetSummary::default()
        };

        match import_single_sheet(
            &sheet,
            &range,
            factions,
            species,
            &mut sheet_summary,
            &mut edge_sources,
        ) {
            Ok(edges) => {
                summary.imported_edges.extend(edges);
                summary.edges_imported += sheet_summary.imported_edges;
                summary.blank_or_zero_cells += sheet_summary.blank_or_zero_cells;
                summary.sheets_imported += 1;
            }
            Err(SheetImportError::Fatal(err)) => return Err(err),
            Err(SheetImportError::AbortSheet(message)) => {
                sheet_summary.aborted = true;
                sheet_summary.errors.push(message.clone());
                summary.errors.push(format!("sheet `{sheet}`: {message}"));
                summary.sheets_aborted += 1;
            }
        }

        summary
            .warnings
            .extend(sheet_summary.warnings.iter().cloned());
        summary.sheet_summaries.push(sheet_summary);
    }

    let catalog = AuthoredRelationshipCatalog::from_edges(
        edge_sources
            .into_iter()
            .map(|(key, (value, _sheet))| (key, value)),
    )
    .map_err(|err| {
        DataImportError::WorkbookOpen(format!("authored relationship catalog build failed: {err}"))
    })?;

    Ok((catalog, summary))
}

enum SheetImportError {
    AbortSheet(String),
    Fatal(DataImportError),
}

fn import_single_sheet(
    sheet: &str,
    range: &calamine::Range<calamine::Data>,
    factions: &FactionCatalog,
    species: &SpeciesCatalog,
    sheet_summary: &mut RelationshipMatrixSheetSummary,
    edge_sources: &mut HashMap<DirectedRelationshipEdgeKey, (i32, String)>,
) -> Result<Vec<ImportedRelationshipEdge>, SheetImportError> {
    let a1 = range_cell(range, 0, 0);
    let direction = MatrixDirection::parse_a1(&a1).map_err(|err| {
        sheet_summary.aborted = true;
        SheetImportError::AbortSheet(err)
    })?;
    sheet_summary.direction = Some(direction);

    let column_headers = collect_column_headers(range).map_err(SheetImportError::AbortSheet)?;
    let row_headers = collect_row_headers(range).map_err(SheetImportError::AbortSheet)?;

    validate_unique_headers(sheet, "column", &column_headers)
        .map_err(SheetImportError::AbortSheet)?;
    validate_unique_headers(sheet, "row", &row_headers).map_err(SheetImportError::AbortSheet)?;

    let valid_columns = resolve_header_axis(
        direction.target,
        &column_headers,
        factions,
        species,
        sheet_summary,
        true,
    );
    let valid_rows = resolve_header_axis(
        direction.source,
        &row_headers,
        factions,
        species,
        sheet_summary,
        false,
    );

    let mut imported = Vec::new();
    for (row_idx, source) in &valid_rows {
        for (col_idx, target) in &valid_columns {
            let value_text = range_cell(range, *row_idx, *col_idx);
            let row_number = row_idx + 1;
            let value = match parse_relationship_cell(&value_text) {
                Ok(Some(value)) => value,
                Ok(None) => {
                    sheet_summary.blank_or_zero_cells += 1;
                    continue;
                }
                Err(message) => {
                    sheet_summary
                        .warnings
                        .push(format!("row {row_number} col {}: {message}", col_idx + 1));
                    continue;
                }
            };

            let key = DirectedRelationshipEdgeKey::new(source.clone(), target.clone());
            if let Some((_, first_sheet)) = edge_sources.get(&key) {
                return Err(SheetImportError::Fatal(
                    DataImportError::DuplicateAuthoredRelationshipEdge {
                        description: format!(
                            "{} already authored on sheet `{first_sheet}` (duplicate on `{sheet}`)",
                            key.prose_direction()
                        ),
                    },
                ));
            }
            edge_sources.insert(key.clone(), (value, sheet.to_string()));
            sheet_summary.imported_edges += 1;
            imported.push(ImportedRelationshipEdge {
                sheet: sheet.to_string(),
                source: source.clone(),
                target: target.clone(),
                value,
            });
        }
    }

    Ok(imported)
}

fn collect_column_headers(
    range: &calamine::Range<calamine::Data>,
) -> Result<Vec<(u32, String)>, String> {
    let mut headers = Vec::new();
    let mut col = 1u32;
    loop {
        let text = range_cell(range, 0, col);
        if text.trim().is_empty() {
            break;
        }
        headers.push((col, text.trim().to_string()));
        col += 1;
    }
    if headers.is_empty() {
        return Err("matrix has no target column headers on row 1".to_string());
    }
    Ok(headers)
}

fn collect_row_headers(
    range: &calamine::Range<calamine::Data>,
) -> Result<Vec<(u32, String)>, String> {
    let mut headers = Vec::new();
    let mut row = 1u32;
    loop {
        let text = range_cell(range, row, 0);
        if text.trim().is_empty() {
            break;
        }
        headers.push((row, text.trim().to_string()));
        row += 1;
    }
    if headers.is_empty() {
        return Err("matrix has no source row headers in column A".to_string());
    }
    Ok(headers)
}

fn validate_unique_headers(
    sheet: &str,
    axis: &str,
    headers: &[(u32, String)],
) -> Result<(), String> {
    let mut seen: HashMap<String, u32> = HashMap::new();
    for (index, raw) in headers {
        let normalized = normalize_relationship_key(raw)
            .map_err(|err| format!("{axis} header `{raw}` on sheet `{sheet}`: {err}"))?;
        if let Some(first_index) = seen.insert(normalized.clone(), *index) {
            return Err(format!(
                "duplicate {axis} id `{normalized}` on sheet `{sheet}` (positions {first_index} and {index})"
            ));
        }
    }
    Ok(())
}

fn resolve_header_axis(
    domain: crate::world::relationship::RelationshipMatrixDomain,
    headers: &[(u32, String)],
    factions: &FactionCatalog,
    species: &SpeciesCatalog,
    sheet_summary: &mut RelationshipMatrixSheetSummary,
    is_column_axis: bool,
) -> Vec<(u32, AuthoredFacetKey)> {
    let mut resolved = Vec::new();
    for (index, raw) in headers {
        match resolve_facet_key(domain, raw, factions, species) {
            Ok(key) => resolved.push((*index, key)),
            Err(message) => {
                sheet_summary.warnings.push(format!(
                    "{} header `{}`: {message}",
                    if is_column_axis { "column" } else { "row" },
                    raw
                ));
                if is_column_axis {
                    sheet_summary.skipped_columns += 1;
                } else {
                    sheet_summary.skipped_rows += 1;
                }
            }
        }
    }
    resolved
}

fn resolve_facet_key(
    domain: crate::world::relationship::RelationshipMatrixDomain,
    raw: &str,
    factions: &FactionCatalog,
    species: &SpeciesCatalog,
) -> Result<AuthoredFacetKey, String> {
    let key = normalize_relationship_key(raw)?;
    match domain {
        crate::world::relationship::RelationshipMatrixDomain::Faction => {
            let id = crate::world::FactionId::new(key);
            if !factions.contains(&id) {
                return Err(format!("unknown Faction key `{id}`"));
            }
            Ok(AuthoredFacetKey::Faction(id))
        }
        crate::world::relationship::RelationshipMatrixDomain::Species => {
            let id = crate::world::SpeciesId::new(key);
            if !species.contains(&id) {
                return Err(format!("unknown Species key `{id}`"));
            }
            Ok(AuthoredFacetKey::Species(id))
        }
    }
}

pub fn parse_relationship_cell(value: &str) -> Result<Option<i32>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "0" {
        return Ok(None);
    }
    trimmed
        .parse::<i32>()
        .map(Some)
        .map_err(|_| format!("expected signed integer, got `{trimmed}`"))
}

pub fn export_relationship_matrix_report_markdown(
    path: &Path,
    summary: &RelationshipMatrixImportSummary,
    catalog: &AuthoredRelationshipCatalog,
) -> std::io::Result<()> {
    use std::io::Write;

    let mut file = std::fs::File::create(path)?;
    writeln!(file, "# Relationship Matrix Import Report")?;
    writeln!(file)?;
    writeln!(
        file,
        "Sheets discovered: {} | imported: {} | aborted: {} | authored edges: {}",
        summary.sheets_discovered,
        summary.sheets_imported,
        summary.sheets_aborted,
        summary.edges_imported
    )?;
    writeln!(
        file,
        "Blank/zero cells skipped: {}",
        summary.blank_or_zero_cells
    )?;
    writeln!(file)?;

    if !summary.errors.is_empty() {
        writeln!(file, "## Errors")?;
        for error in &summary.errors {
            writeln!(file, "- {error}")?;
        }
        writeln!(file)?;
    }

    if !summary.warnings.is_empty() {
        writeln!(file, "## Warnings")?;
        for warning in &summary.warnings {
            writeln!(file, "- {warning}")?;
        }
        writeln!(file)?;
    }

    writeln!(file, "## Sheets")?;
    for sheet in &summary.sheet_summaries {
        let direction = sheet
            .direction
            .map(|direction| {
                format!(
                    "{} -> {}",
                    direction.source.label(),
                    direction.target.label()
                )
            })
            .unwrap_or_else(|| "(invalid)".to_string());
        writeln!(
            file,
            "### `{}` — {direction} (edges={}, blank/zero={}, skipped rows={}, skipped cols={})",
            sheet.sheet,
            sheet.imported_edges,
            sheet.blank_or_zero_cells,
            sheet.skipped_rows,
            sheet.skipped_columns
        )?;
        for warning in &sheet.warnings {
            writeln!(file, "- warning: {warning}")?;
        }
        for error in &sheet.errors {
            writeln!(file, "- error: {error}")?;
        }
        writeln!(file)?;
    }

    writeln!(file, "## Authored edges")?;
    if catalog.is_empty() {
        writeln!(file, "_No authored edges imported._")?;
    } else {
        for (key, value) in catalog.sorted_edges() {
            writeln!(file, "- {} = {value}", key.prose_direction())?;
        }
    }

    Ok(())
}

#[cfg(all(feature = "data-import", test))]
mod integration_tests {
    use super::*;
    use crate::data_import::relationship::{
        faction_excel::FACTIONS_SHEET_NAME, import_faction_catalog_from_excel,
        import_species_catalog_from_excel, species_excel::SPECIES_SHEET_NAME,
    };
    use rust_xlsxwriter::Workbook;
    use std::path::PathBuf;

    fn write_matrix_sheet(path: &Path, sheet: &str, rows: &[Vec<&str>]) {
        let mut workbook = if path.exists() {
            // Append to existing workbook by recreating with multiple sheets via temp approach
            Workbook::new()
        } else {
            Workbook::new()
        };
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(sheet).unwrap();
        for (row_idx, row) in rows.iter().enumerate() {
            for (col_idx, value) in row.iter().enumerate() {
                worksheet
                    .write_string(row_idx as u32, col_idx as u16, *value)
                    .unwrap();
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        workbook.save(path).unwrap();
    }

    fn write_workbook_with_sheets(path: &Path, sheets: &[(&str, Vec<Vec<&str>>)]) {
        let mut workbook = Workbook::new();
        for (name, rows) in sheets {
            let worksheet = workbook.add_worksheet();
            worksheet.set_name(*name).unwrap();
            for (row_idx, row) in rows.iter().enumerate() {
                for (col_idx, value) in row.iter().enumerate() {
                    worksheet
                        .write_string(row_idx as u32, col_idx as u16, *value)
                        .unwrap();
                }
            }
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        workbook.save(path).unwrap();
    }

    fn write_identity_sheets(path: &Path) {
        write_workbook_with_sheets(
            path,
            &[
                (
                    FACTIONS_SHEET_NAME,
                    vec![
                        vec!["Faction Key", "Name", "Enabled"],
                        vec!["player", "Player", "Y"],
                        vec!["wild", "Wild", "Y"],
                    ],
                ),
                (
                    SPECIES_SHEET_NAME,
                    vec![
                        vec!["Species Key", "Name", "Enabled"],
                        vec!["wolf", "Wolf", "Y"],
                        vec!["deer", "Deer", "Y"],
                    ],
                ),
            ],
        );
    }

    fn temp_workbook(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "chasma_rel_matrix_{}_{}.xlsx",
            std::process::id(),
            name
        ))
    }

    fn catalogs(path: &Path) -> (FactionCatalog, SpeciesCatalog) {
        let (factions, _) = import_faction_catalog_from_excel(path).unwrap();
        let (species, _) = import_species_catalog_from_excel(path).unwrap();
        (factions, species)
    }

    fn faction_faction_sheet() -> Vec<Vec<&'static str>> {
        vec![
            vec!["Faction -> Faction", "player", "wild"],
            vec!["player", "0", "-100"],
            vec!["wild", "50", "0"],
        ]
    }

    #[test]
    fn imports_faction_to_faction_matrix() {
        let path = temp_workbook("ff");
        write_identity_sheets(&path);
        write_workbook_with_sheets(&path, &[("Rel Faction Faction", faction_faction_sheet())]);
        // Re-write combined - write_identity then append matrix by full rewrite
        write_workbook_with_sheets(
            &path,
            &[
                (
                    FACTIONS_SHEET_NAME,
                    vec![
                        vec!["Faction Key", "Name", "Enabled"],
                        vec!["player", "Player", "Y"],
                        vec!["wild", "Wild", "Y"],
                    ],
                ),
                (
                    SPECIES_SHEET_NAME,
                    vec![
                        vec!["Species Key", "Name", "Enabled"],
                        vec!["wolf", "Wolf", "Y"],
                        vec!["deer", "Deer", "Y"],
                    ],
                ),
                ("Rel Faction Faction", faction_faction_sheet()),
            ],
        );
        let (factions, species) = catalogs(&path);
        let (catalog, summary) =
            import_authored_relationship_matrices_from_excel(&path, &factions, &species).unwrap();
        assert_eq!(summary.edges_imported, 2);
        assert_eq!(
            catalog.get_edge(
                &AuthoredFacetKey::Faction(crate::world::FactionId::new("wild")),
                &AuthoredFacetKey::Faction(crate::world::FactionId::new("player")),
            ),
            Some(50)
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn asymmetric_directions_are_independent() {
        let path = temp_workbook("asym");
        write_workbook_with_sheets(
            &path,
            &[
                (
                    FACTIONS_SHEET_NAME,
                    vec![
                        vec!["Faction Key", "Name", "Enabled"],
                        vec!["player", "Player", "Y"],
                        vec!["wild", "Wild", "Y"],
                    ],
                ),
                (
                    SPECIES_SHEET_NAME,
                    vec![
                        vec!["Species Key", "Name", "Enabled"],
                        vec!["wolf", "Wolf", "Y"],
                    ],
                ),
                ("Rel Faction Faction", faction_faction_sheet()),
            ],
        );
        let (factions, species) = catalogs(&path);
        let (catalog, _) =
            import_authored_relationship_matrices_from_excel(&path, &factions, &species).unwrap();
        assert_eq!(
            catalog.get_edge(
                &AuthoredFacetKey::Faction(crate::world::FactionId::new("player")),
                &AuthoredFacetKey::Faction(crate::world::FactionId::new("wild")),
            ),
            Some(-100)
        );
        assert_eq!(
            catalog.get_edge(
                &AuthoredFacetKey::Faction(crate::world::FactionId::new("wild")),
                &AuthoredFacetKey::Faction(crate::world::FactionId::new("player")),
            ),
            Some(50)
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn imports_all_four_domain_combinations_with_one_parser() {
        let path = temp_workbook("all4");
        write_workbook_with_sheets(
            &path,
            &[
                (
                    FACTIONS_SHEET_NAME,
                    vec![
                        vec!["Faction Key", "Name", "Enabled"],
                        vec!["player", "Player", "Y"],
                        vec!["wild", "Wild", "Y"],
                    ],
                ),
                (
                    SPECIES_SHEET_NAME,
                    vec![
                        vec!["Species Key", "Name", "Enabled"],
                        vec!["wolf", "Wolf", "Y"],
                        vec!["deer", "Deer", "Y"],
                    ],
                ),
                ("Rel Faction Faction", faction_faction_sheet()),
                (
                    "Rel Faction Species",
                    vec![
                        vec!["Faction -> Species", "wolf", "deer"],
                        vec!["player", "25", ""],
                        vec!["wild", "", "-10"],
                    ],
                ),
                (
                    "Rel Species Faction",
                    vec![
                        vec!["Species -> Faction", "player", "wild"],
                        vec!["wolf", "5", ""],
                    ],
                ),
                (
                    "Rel Species Species",
                    vec![
                        vec!["Species -> Species", "wolf", "deer"],
                        vec!["wolf", "", "-5"],
                    ],
                ),
            ],
        );
        let (factions, species) = catalogs(&path);
        let (catalog, summary) =
            import_authored_relationship_matrices_from_excel(&path, &factions, &species).unwrap();
        assert_eq!(summary.sheets_imported, 4);
        assert_eq!(summary.edges_imported, 6);
        assert_eq!(
            catalog.get_edge(
                &AuthoredFacetKey::Faction(crate::world::FactionId::new("player")),
                &AuthoredFacetKey::Species(crate::world::SpeciesId::new("wolf")),
            ),
            Some(25)
        );
        assert_eq!(
            catalog.get_edge(
                &AuthoredFacetKey::Species(crate::world::SpeciesId::new("wolf")),
                &AuthoredFacetKey::Faction(crate::world::FactionId::new("player")),
            ),
            Some(5)
        );
        assert_eq!(
            catalog.get_edge(
                &AuthoredFacetKey::Species(crate::world::SpeciesId::new("wolf")),
                &AuthoredFacetKey::Species(crate::world::SpeciesId::new("deer")),
            ),
            Some(-5)
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn blank_and_zero_cells_store_no_edge() {
        let path = temp_workbook("blank");
        write_workbook_with_sheets(
            &path,
            &[
                (
                    FACTIONS_SHEET_NAME,
                    vec![
                        vec!["Faction Key", "Name", "Enabled"],
                        vec!["player", "Player", "Y"],
                        vec!["wild", "Wild", "Y"],
                    ],
                ),
                (
                    SPECIES_SHEET_NAME,
                    vec![
                        vec!["Species Key", "Name", "Enabled"],
                        vec!["wolf", "Wolf", "Y"],
                    ],
                ),
                (
                    "Rel Faction Faction",
                    vec![
                        vec!["Faction -> Faction", "player", "wild"],
                        vec!["player", "", "0"],
                        vec!["wild", "0", ""],
                    ],
                ),
            ],
        );
        let (factions, species) = catalogs(&path);
        let (catalog, summary) =
            import_authored_relationship_matrices_from_excel(&path, &factions, &species).unwrap();
        assert_eq!(catalog.len(), 0);
        assert_eq!(summary.blank_or_zero_cells, 4);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn duplicate_directed_edge_across_sheets_hard_fails() {
        let path = temp_workbook("dup");
        write_workbook_with_sheets(
            &path,
            &[
                (
                    FACTIONS_SHEET_NAME,
                    vec![
                        vec!["Faction Key", "Name", "Enabled"],
                        vec!["player", "Player", "Y"],
                        vec!["wild", "Wild", "Y"],
                    ],
                ),
                (
                    SPECIES_SHEET_NAME,
                    vec![
                        vec!["Species Key", "Name", "Enabled"],
                        vec!["wolf", "Wolf", "Y"],
                    ],
                ),
                ("Rel Faction Faction", faction_faction_sheet()),
                (
                    "Rel Faction Faction Copy",
                    vec![
                        vec!["Faction -> Faction", "player", "wild"],
                        vec!["wild", "99", ""],
                    ],
                ),
            ],
        );
        let (factions, species) = catalogs(&path);
        let err = import_authored_relationship_matrices_from_excel(&path, &factions, &species)
            .unwrap_err();
        assert!(matches!(
            err,
            DataImportError::DuplicateAuthoredRelationshipEdge { .. }
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn unknown_header_ids_are_skipped_with_warnings() {
        let path = temp_workbook("unknown");
        write_workbook_with_sheets(
            &path,
            &[
                (
                    FACTIONS_SHEET_NAME,
                    vec![
                        vec!["Faction Key", "Name", "Enabled"],
                        vec!["player", "Player", "Y"],
                    ],
                ),
                (
                    SPECIES_SHEET_NAME,
                    vec![
                        vec!["Species Key", "Name", "Enabled"],
                        vec!["wolf", "Wolf", "Y"],
                    ],
                ),
                (
                    "Rel Faction Faction",
                    vec![
                        vec!["Faction -> Faction", "player", "missing_faction"],
                        vec!["player", "10", ""],
                        vec!["missing_faction", "", "20"],
                    ],
                ),
            ],
        );
        let (factions, species) = catalogs(&path);
        let (catalog, summary) =
            import_authored_relationship_matrices_from_excel(&path, &factions, &species).unwrap();
        assert_eq!(catalog.len(), 1);
        assert!(
            summary
                .warnings
                .iter()
                .any(|w| w.contains("missing_faction"))
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn discovers_rel_prefixed_sheets_only() {
        let path = temp_workbook("discover");
        write_workbook_with_sheets(
            &path,
            &[
                (
                    FACTIONS_SHEET_NAME,
                    vec![
                        vec!["Faction Key", "Name", "Enabled"],
                        vec!["player", "Player", "Y"],
                    ],
                ),
                (
                    SPECIES_SHEET_NAME,
                    vec![
                        vec!["Species Key", "Name", "Enabled"],
                        vec!["wolf", "Wolf", "Y"],
                    ],
                ),
                ("Not Rel Sheet", vec![vec!["ignored", "ignored"]]),
                (
                    "Rel Faction Faction",
                    vec![vec!["Faction -> Faction", "player"], vec!["player", "0"]],
                ),
            ],
        );
        let names = discover_relationship_matrix_sheet_names(&path).unwrap();
        assert_eq!(names, vec!["Rel Faction Faction".to_string()]);
        let _ = std::fs::remove_file(path);
    }
}
