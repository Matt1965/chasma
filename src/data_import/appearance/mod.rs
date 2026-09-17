//! Appearance profile Excel import (CG1).

#[cfg(feature = "data-import")]
mod dev_load;
#[cfg(feature = "data-import")]
mod excel;
mod schema;

pub use schema::{
    AppearanceBodyVariantImportRow, AppearanceMorphMappingImportRow,
    AppearanceParameterImportRow, AppearanceProfileImportRow, MORPH_MAPPING_REQUIRED_COLUMNS,
    PARAMETER_REQUIRED_COLUMNS, PROFILE_REQUIRED_COLUMNS, VARIANT_REQUIRED_COLUMNS,
};

#[cfg(feature = "data-import")]
pub use dev_load::resolve_dev_appearance_profile_catalog;
#[cfg(feature = "data-import")]
pub use excel::{
    APPEARANCE_BODY_VARIANTS_SHEET_NAME, APPEARANCE_MORPH_MAPPINGS_SHEET_NAME,
    APPEARANCE_PARAMETERS_SHEET_NAME, APPEARANCE_PROFILES_SHEET_NAME,
};

#[cfg(feature = "data-import")]
pub fn import_appearance_profiles_from_excel(
    path: &std::path::Path,
) -> Result<
    (
        crate::world::AppearanceProfileCatalog,
        crate::data_import::ImportSummary,
    ),
    crate::data_import::DataImportError,
> {
    use excel::{
        read_appearance_body_variant_rows, read_appearance_morph_mapping_rows,
        read_appearance_parameter_rows, read_appearance_profile_rows,
    };
    use schema::assemble_profiles;

    let profile_rows_result = read_appearance_profile_rows(path)?;
    let variant_rows_result = read_appearance_body_variant_rows(path)?;
    let parameter_rows_result = read_appearance_parameter_rows(path)?;
    let mapping_rows_result = read_appearance_morph_mapping_rows(path)?;

    let mut summary = crate::data_import::ImportSummary::default();
    summary.rows_processed = profile_rows_result.len()
        + variant_rows_result.len()
        + parameter_rows_result.len()
        + mapping_rows_result.len();

    let mut profile_rows = Vec::new();
    for row_result in profile_rows_result {
        match row_result {
            Ok(mut row) => {
                row.row_number = summary.rows_valid + summary.rows_failed + 2;
                profile_rows.push(row);
                summary.rows_valid += 1;
            }
            Err(row_err) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("profile row {}: {}", row_err.row_number, row_err.message));
            }
        }
    }

    let mut variant_rows = Vec::new();
    for row_result in variant_rows_result {
        match row_result {
            Ok(row) => {
                variant_rows.push(row);
                summary.rows_valid += 1;
            }
            Err(row_err) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("variant row {}: {}", row_err.row_number, row_err.message));
            }
        }
    }

    let mut parameter_rows = Vec::new();
    for row_result in parameter_rows_result {
        match row_result {
            Ok(row) => {
                parameter_rows.push(row);
                summary.rows_valid += 1;
            }
            Err(row_err) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("parameter row {}: {}", row_err.row_number, row_err.message));
            }
        }
    }

    if profile_rows.is_empty() {
        return Err(crate::data_import::DataImportError::NoValidRows);
    }

    let mut mapping_rows = Vec::new();
    for row_result in mapping_rows_result {
        match row_result {
            Ok(row) => {
                mapping_rows.push(row);
                summary.rows_valid += 1;
            }
            Err(row_err) => {
                summary.rows_failed += 1;
                summary
                    .warnings
                    .push(format!("morph mapping row {}: {}", row_err.row_number, row_err.message));
            }
        }
    }

    let definitions =
        assemble_profiles(profile_rows, variant_rows, parameter_rows, mapping_rows).map_err(
            |message| crate::data_import::DataImportError::WorkbookOpen(message),
        )?;

    let catalog = crate::world::AppearanceProfileCatalog::from_definitions(definitions).map_err(
        |err| {
            crate::data_import::DataImportError::WorkbookOpen(format!(
                "appearance profile catalog build failed: {err:?}"
            ))
        },
    )?;

    Ok((catalog, summary))
}

#[cfg(test)]
mod tests;
