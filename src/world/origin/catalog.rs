use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::OriginDefinition;
use super::id::OriginId;

/// Catalog of authored starting origins (CG7).
#[derive(Debug, Clone, Default, Resource, Reflect)]
pub struct OriginCatalog {
    definitions: Vec<OriginDefinition>,
    by_id: HashMap<OriginId, usize>,
}

impl OriginCatalog {
    pub fn from_definitions(definitions: Vec<OriginDefinition>) -> Result<Self, String> {
        let mut by_id = HashMap::new();
        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(format!("duplicate origin id `{}`", definition.id.as_str()));
            }
            if definition.roster.is_empty() {
                return Err(format!(
                    "origin `{}` must include at least one roster member",
                    definition.id.as_str()
                ));
            }
        }
        Ok(Self {
            definitions,
            by_id,
        })
    }

    pub fn definitions(&self) -> &[OriginDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &OriginId) -> Option<&OriginDefinition> {
        self.by_id.get(id).map(|index| &self.definitions[*index])
    }

    pub fn get_index(&self, index: usize) -> Option<&OriginDefinition> {
        self.definitions.get(index)
    }
}
