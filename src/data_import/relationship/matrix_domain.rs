//! Parse relationship matrix sheet direction declarations (`A1`).

pub use crate::world::relationship::{MatrixDirection, RelationshipMatrixDomain};

pub fn parse_matrix_a1(text: &str) -> Result<MatrixDirection, String> {
    MatrixDirection::parse_a1(text)
}
