use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::OriginDefinition;
use super::id::OriginId;

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
            if definition.members.is_empty() {
                return Err(format!(
                    "origin `{}` must include at least one member",
                    definition.id.as_str()
                ));
            }
        }
        Ok(Self { definitions, by_id })
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

    pub fn upsert(&mut self, definition: OriginDefinition) -> Result<(), String> {
        if definition.members.is_empty() {
            return Err(format!(
                "origin `{}` must include at least one member",
                definition.id.as_str()
            ));
        }
        if let Some(index) = self.by_id.get(&definition.id).copied() {
            self.definitions[index] = definition;
            return Ok(());
        }
        let index = self.definitions.len();
        self.by_id.insert(definition.id.clone(), index);
        self.definitions.push(definition);
        Ok(())
    }

    pub fn remove(&mut self, id: &OriginId) -> bool {
        let Some(index) = self.by_id.remove(id) else {
            return false;
        };
        self.definitions.remove(index);
        self.rebuild_index();
        true
    }

    pub fn replace_all(&mut self, definitions: Vec<OriginDefinition>) -> Result<(), String> {
        *self = Self::from_definitions(definitions)?;
        Ok(())
    }

    fn rebuild_index(&mut self) {
        self.by_id.clear();
        for (index, definition) in self.definitions.iter().enumerate() {
            self.by_id.insert(definition.id.clone(), index);
        }
    }
}
