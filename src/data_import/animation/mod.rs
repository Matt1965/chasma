//! Animation profile Excel import (A1).

#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
mod excel;
mod schema;
mod validate;

pub use schema::{AnimationProfileImportRow, OPTIONAL_COLUMNS, REQUIRED_COLUMNS};

#[cfg(feature = "data-import")]
pub use dev_load::resolve_dev_animation_profile_catalog;
#[cfg(feature = "data-import")]
pub use excel::ANIMATION_PROFILES_SHEET_NAME;

#[cfg(feature = "data-import")]
pub fn import_animation_profiles_from_excel(
    path: &std::path::Path,
) -> Result<
    (
        Vec<crate::world::AnimationProfile>,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    use std::collections::HashMap;

    use crate::world::AnimationProfileId;

    use excel::read_animation_profile_rows;
    use validate::validate_row;

    let rows = read_animation_profile_rows(path)?;
    let mut summary = crate::data_import::ImportSummary {
        rows_processed: rows.len(),
        ..crate::data_import::ImportSummary::default()
    };
    let mut definitions = Vec::new();
    let mut seen_ids: HashMap<AnimationProfileId, usize> = HashMap::new();

    for row_result in rows {
        let row = match row_result {
            Ok(row) => row,
            Err(row_err) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("row {}: {}", row_err.row_number, row_err.message));
                continue;
            }
        };

        if let Err(row_err) = validate_row(&row) {
            summary.rows_failed += 1;
            summary
                .warnings
                .push(format!("row {}: {}", row_err.row_number, row_err.message));
            continue;
        }

        if !row.enabled {
            summary.warnings.push(format!(
                "row {}: Enabled=false — profile excluded from catalog",
                row.row_number
            ));
            continue;
        }

        let definition = row.to_definition();
        let id = definition.id.clone();
        if let Some(first_row) = seen_ids.insert(id.clone(), row.row_number) {
            return Err(
                crate::data_import::DataImportError::DuplicateAnimationProfileId {
                    id,
                    first_row,
                    duplicate_row: row.row_number,
                },
            );
        }

        if row.enabled_was_blank {
            summary.warnings.push(format!(
                "row {}: Enabled blank — defaulting to true",
                row.row_number
            ));
        }

        definitions.push(definition);
        summary.rows_valid += 1;
    }

    if summary.rows_valid == 0 {
        return Err(crate::data_import::DataImportError::NoValidRows);
    }

    Ok((definitions, summary))
}
