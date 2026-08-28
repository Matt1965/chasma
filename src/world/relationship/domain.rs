//! Catalog-backed relationship matrix domains (ADR-132 Phase 2).

use bevy::prelude::*;

/// A domain that may appear in authored Excel relationship matrices.///
/// `Individual` is intentionally excluded — unit ids are not matrix-authored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub enum RelationshipMatrixDomain {
    Faction,
    Species,
}

impl RelationshipMatrixDomain {
    pub fn label(self) -> &'static str {
        match self {
            Self::Faction => "Faction",
            Self::Species => "Species",
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "faction" => Ok(Self::Faction),
            "species" => Ok(Self::Species),
            other => Err(format!("unknown relationship domain `{other}`")),
        }
    }
}

/// Declared direction of one relationship matrix sheet (`A1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub struct MatrixDirection {
    pub source: RelationshipMatrixDomain,
    pub target: RelationshipMatrixDomain,
}

impl MatrixDirection {
    pub fn parse_a1(text: &str) -> Result<Self, String> {
        let trimmed = text.trim();
        let Some((source_text, target_text)) = trimmed.split_once("->") else {
            return Err(format!(
                "A1 must declare direction as `Source -> Target`, got `{trimmed}`"
            ));
        };
        Ok(Self {
            source: RelationshipMatrixDomain::parse(source_text)?,
            target: RelationshipMatrixDomain::parse(target_text)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a1_direction() {
        let direction = MatrixDirection::parse_a1("Faction -> Species").unwrap();
        assert_eq!(direction.source, RelationshipMatrixDomain::Faction);
        assert_eq!(direction.target, RelationshipMatrixDomain::Species);
    }

    #[test]
    fn rejects_unknown_domain() {
        assert!(MatrixDirection::parse_a1("Individual -> Faction").is_err());
    }
}
