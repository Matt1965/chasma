//! Terrain field import validation (ADR-101 TF1).

use crate::world::TerrainFieldDefinition;

use super::schema::TerrainFieldImportRow;

pub fn validate_row(row: &TerrainFieldImportRow) -> Result<TerrainFieldDefinition, String> {
    if row.field_id.trim().is_empty() {
        return Err("Terrain Field ID is required".to_string());
    }
    if row.name.trim().is_empty() {
        return Err("Name is required".to_string());
    }
    row.to_definition()
}
