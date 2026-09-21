use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::equipment::EquipmentSlot;
use crate::world::relationship::SpeciesId;
use crate::world::Affiliation;
use crate::world::{ItemDefinitionId, UnitCatalog, UnitDefinitionId};

/// Stable key for a unit spawn archetype preset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct UnitArchetypeId(pub String);

impl UnitArchetypeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One equipped item authored by an archetype.
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct ArchetypeEquipmentEntry {
    pub item_id: ItemDefinitionId,
    pub slot: EquipmentSlot,
}

/// One inventory stack authored by an archetype (personal grid, not equipment slots).
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub struct ArchetypeInventoryStack {
    pub item_id: ItemDefinitionId,
    pub quantity: u32,
}

/// Editor-authored role/loadout template layered onto a base unit definition.
#[derive(Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct UnitArchetypeDefinition {
    pub id: UnitArchetypeId,
    pub display_name: String,
    pub applicable_species: Vec<SpeciesId>,
    pub gold_min: u32,
    pub gold_max: u32,
    pub affiliation_override: Option<Affiliation>,
    pub equipment: Vec<ArchetypeEquipmentEntry>,
    pub inventory_stacks: Vec<ArchetypeInventoryStack>,
    pub enabled: bool,
}

impl UnitArchetypeDefinition {
    pub fn applies_to_species(&self, species_id: &SpeciesId) -> bool {
        self.applicable_species.iter().any(|id| id == species_id)
    }

    pub fn applies_to_unit(
        &self,
        unit_id: &UnitDefinitionId,
        unit_catalog: &UnitCatalog,
    ) -> bool {
        unit_catalog
            .get(unit_id)
            .is_some_and(|definition| self.applies_to_species(&definition.species_id))
    }

    pub fn validate_gold_range(&self) -> Result<(), String> {
        validate_gold_range(self.gold_min, self.gold_max)
    }
}

pub fn validate_gold_range(gold_min: u32, gold_max: u32) -> Result<(), String> {
    if gold_min > gold_max {
        return Err("gold min must not exceed gold max".to_string());
    }
    Ok(())
}

/// Read-only registry of unit archetype presets.
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct UnitArchetypeCatalog {
    definitions: Vec<UnitArchetypeDefinition>,
    by_id: HashMap<UnitArchetypeId, usize>,
}

impl Default for UnitArchetypeCatalog {
    fn default() -> Self {
        Self {
            definitions: Vec::new(),
            by_id: HashMap::new(),
        }
    }
}

impl UnitArchetypeCatalog {
    pub fn from_definitions(
        definitions: Vec<UnitArchetypeDefinition>,
    ) -> Result<Self, UnitArchetypeCatalogError> {
        let mut by_id = HashMap::with_capacity(definitions.len());
        for (index, definition) in definitions.iter().enumerate() {
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(UnitArchetypeCatalogError::DuplicateId(definition.id.clone()));
            }
            definition.validate_gold_range().map_err(|message| {
                UnitArchetypeCatalogError::InvalidDefinition {
                    id: definition.id.clone(),
                    message,
                }
            })?;
        }
        Ok(Self { definitions, by_id })
    }

    pub fn definitions(&self) -> &[UnitArchetypeDefinition] {
        &self.definitions
    }

    pub fn get(&self, id: &UnitArchetypeId) -> Option<&UnitArchetypeDefinition> {
        self.by_id.get(id).map(|&index| &self.definitions[index])
    }

    pub fn upsert(
        &mut self,
        definition: UnitArchetypeDefinition,
    ) -> Result<(), UnitArchetypeCatalogError> {
        definition.validate_gold_range().map_err(|message| {
            UnitArchetypeCatalogError::InvalidDefinition {
                id: definition.id.clone(),
                message,
            }
        })?;
        if let Some(&index) = self.by_id.get(&definition.id) {
            self.definitions[index] = definition;
            return Ok(());
        }
        if self.by_id.contains_key(&definition.id) {
            return Err(UnitArchetypeCatalogError::DuplicateId(definition.id.clone()));
        }
        let index = self.definitions.len();
        self.by_id.insert(definition.id.clone(), index);
        self.definitions.push(definition);
        Ok(())
    }

    pub fn remove(&mut self, id: &UnitArchetypeId) -> bool {
        let Some(index) = self.by_id.remove(id) else {
            return false;
        };
        self.definitions.remove(index);
        self.by_id.clear();
        for (new_index, definition) in self.definitions.iter().enumerate() {
            self.by_id.insert(definition.id.clone(), new_index);
        }
        true
    }

    pub fn archetypes_for_unit(
        &self,
        unit_id: &UnitDefinitionId,
        unit_catalog: &UnitCatalog,
        enabled_only: bool,
    ) -> Vec<&UnitArchetypeDefinition> {
        self.definitions
            .iter()
            .filter(|definition| {
                (!enabled_only || definition.enabled)
                    && definition.applies_to_unit(unit_id, unit_catalog)
            })
            .collect()
    }

    pub fn display_name_taken(&self, display_name: &str, except_id: Option<&UnitArchetypeId>) -> bool {
        let normalized = display_name.trim().to_ascii_lowercase();
        self.definitions.iter().any(|definition| {
            if except_id.is_some_and(|id| id == &definition.id) {
                return false;
            }
            definition.display_name.trim().to_ascii_lowercase() == normalized
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitArchetypeCatalogError {
    DuplicateId(UnitArchetypeId),
    InvalidDefinition {
        id: UnitArchetypeId,
        message: String,
    },
}

pub fn slugify_archetype_id(display_name: &str) -> String {
    let mut slug = String::new();
    let mut last_was_sep = true;
    for ch in display_name.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_sep = false;
        } else if !last_was_sep {
            slug.push('_');
            last_was_sep = true;
        }
    }
    let trimmed = slug.trim_matches('_');
    if trimmed.is_empty() {
        "archetype".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn unique_unit_archetype_id(display_name: &str, catalog: &UnitArchetypeCatalog) -> UnitArchetypeId {
    let base = slugify_archetype_id(display_name);
    if catalog.get(&UnitArchetypeId::new(&base)).is_none() {
        return UnitArchetypeId::new(base);
    }
    for suffix in 2..10_000 {
        let candidate = format!("{base}_{suffix}");
        if catalog.get(&UnitArchetypeId::new(&candidate)).is_none() {
            return UnitArchetypeId::new(candidate);
        }
    }
    UnitArchetypeId::new(format!("{base}_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)))
}
