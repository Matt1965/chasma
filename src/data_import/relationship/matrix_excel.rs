use calamine::{Data, Range};

use crate::data_import::error::DataImportError;

pub const RELATIONSHIP_MATRIX_SHEET_PREFIX: &str = "Rel ";

pub fn discover_relationship_matrix_sheet_names(
    path: &std::path::Path,
) -> Result<Vec<String>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    Ok(workbook
        .sheet_names()
        .iter()
        .filter(|name| name.starts_with(RELATIONSHIP_MATRIX_SHEET_PREFIX))
        .cloned()
        .collect())
}

pub fn read_relationship_matrix_range(
    path: &std::path::Path,
    sheet_name: &str,
) -> Result<Range<Data>, DataImportError> {
    use calamine::{Reader, Xlsx, XlsxError, open_workbook};

    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|err: XlsxError| DataImportError::WorkbookOpen(err.to_string()))?;
    workbook
        .worksheet_range(sheet_name)
        .map_err(|_| DataImportError::SheetNotFound {
            sheet: sheet_name.to_string(),
        })
}

pub fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::String(value) => value.clone(),
        Data::Float(value) => {
            if value.fract() == 0.0 {
                format!("{}", *value as i64)
            } else {
                value.to_string()
            }
        }
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.clone(),
        Data::DurationIso(value) => value.clone(),
        Data::Error(_) | Data::Empty => String::new(),
    }
}

pub fn range_cell(range: &Range<Data>, row: u32, col: u32) -> String {
    range
        .get((row as usize, col as usize))
        .map(cell_to_string)
        .unwrap_or_default()
}
