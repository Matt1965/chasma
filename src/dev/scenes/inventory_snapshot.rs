//! Inventory-related scene snapshot records (ADR-094 I8).

use serde::{Deserialize, Serialize};

use crate::world::{
    Affiliation, BuildingId, ChunkId, CorpseId, CorpseRecord, CorpseState, InventoryCatalogCtx,
    InventoryEntryContents, InventoryId, InventoryOwnerRef, InventoryProfileId, InventoryRecord,
    ItemDefinitionId, ItemInstance, ItemInstanceId, ItemInstanceLocation, ItemInstanceMetadata,
    ItemPileId, ItemPileSource, OwnerId, PlacedInventoryEntry, SpaceId, TeamId, UnitId,
    UnitPlacement, WorldData, WorldItemPileRecord, WorldPileContents,
    rebuild_all_inventory_derived,
};

use super::snapshot::{SceneQuat, SceneRecordError, SceneWorldPosition, affiliation_from_label};

fn default_next_inventory_id() -> u32 {
    1
}

fn default_next_item_instance_id() -> u32 {
    1
}

fn default_next_corpse_id() -> u64 {
    1
}

fn default_next_item_pile_id() -> u64 {
    1
}

/// Serializable placed inventory entry (ADR-094 I8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenePlacedEntryRecord {
    pub anchor_x: u8,
    pub anchor_y: u8,
    #[serde(rename = "kind")]
    pub entry_kind: String,
    #[serde(default)]
    pub item_definition_id: Option<String>,
    #[serde(default)]
    pub item_instance_id: Option<u32>,
    #[serde(default)]
    pub quantity: Option<u32>,
}

/// Serializable inventory container (ADR-094 I8). Derived caches omitted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneInventoryRecord {
    pub id: u32,
    pub owner: String,
    pub profile_id: String,
    pub grid_width: u8,
    pub grid_height: u8,
    pub entries: Vec<ScenePlacedEntryRecord>,
}

/// Serializable unique item instance (ADR-094 I8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneItemInstanceRecord {
    pub id: u32,
    pub definition_id: String,
    #[serde(default)]
    pub quality: Option<u32>,
}

/// Serializable instance location (ADR-094 I8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneItemInstanceLocationRecord {
    pub instance_id: u32,
    pub location_kind: String,
    #[serde(default)]
    pub inventory_id: Option<u32>,
    #[serde(default)]
    pub entry_index: Option<usize>,
    #[serde(default)]
    pub pile_id: Option<u64>,
}

/// Serializable corpse (ADR-094 I8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneCorpseRecord {
    pub id: u64,
    pub origin_unit_id: u64,
    pub unit_definition_id: String,
    pub position: SceneWorldPosition,
    pub rotation: SceneQuat,
    pub current_space_id: u32,
    #[serde(default)]
    pub inventory_id: Option<u32>,
    #[serde(default)]
    pub owner_id: Option<u64>,
    #[serde(default)]
    pub team_id: Option<u64>,
    #[serde(default)]
    pub affiliation: Option<String>,
    pub created_tick: u64,
    pub remaining_lifetime_ticks: u64,
    pub state: String,
}

/// Serializable world item pile (ADR-094 I8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneItemPileRecord {
    pub id: u64,
    pub position: SceneWorldPosition,
    pub current_space_id: u32,
    pub contents_kind: String,
    #[serde(default)]
    pub item_definition_id: Option<String>,
    #[serde(default)]
    pub quantity: Option<u32>,
    #[serde(default)]
    pub item_instance_id: Option<u32>,
    #[serde(default)]
    pub owner_id: Option<u64>,
    #[serde(default)]
    pub team_id: Option<u64>,
    #[serde(default)]
    pub affiliation: Option<String>,
    pub source: String,
    pub created_tick: u64,
}

/// Inventory persistence bundle for scene files (ADR-094 I8).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneInventoryPersistence {
    #[serde(default)]
    pub inventory_records: Vec<SceneInventoryRecord>,
    #[serde(default)]
    pub item_instance_records: Vec<SceneItemInstanceRecord>,
    #[serde(default)]
    pub item_instance_locations: Vec<SceneItemInstanceLocationRecord>,
    #[serde(default)]
    pub corpse_records: Vec<SceneCorpseRecord>,
    #[serde(default)]
    pub item_pile_records: Vec<SceneItemPileRecord>,
    #[serde(default = "default_next_inventory_id")]
    pub next_inventory_id: u32,
    #[serde(default = "default_next_item_instance_id")]
    pub next_item_instance_id: u32,
    #[serde(default = "default_next_corpse_id")]
    pub next_corpse_id: u64,
    #[serde(default = "default_next_item_pile_id")]
    pub next_item_pile_id: u64,
}

pub fn capture_inventory_persistence(world: &WorldData) -> SceneInventoryPersistence {
    let inventory_records = world
        .inventory_store()
        .sorted_inventory_ids()
        .into_iter()
        .map(|id| {
            SceneInventoryRecord::from_record(world.inventory_store().get(id).expect("sorted id"))
        })
        .collect();
    let mut item_instance_records = Vec::new();
    let mut item_instance_locations = Vec::new();
    for id in world.item_instance_store().sorted_item_instance_ids() {
        let instance = world.item_instance_store().get(id).expect("sorted id");
        item_instance_records.push(SceneItemInstanceRecord::from_instance(instance));
        if let Some(location) = world.item_instance_store().location(id) {
            if !location.is_detached() {
                item_instance_locations
                    .push(SceneItemInstanceLocationRecord::from_location(id, location));
            }
        }
    }
    let corpse_records = world
        .corpse_store()
        .sorted_corpse_ids()
        .into_iter()
        .map(|id| SceneCorpseRecord::from_record(world.corpse_store().get(id).expect("sorted id")))
        .collect();
    let item_pile_records = world
        .item_pile_store()
        .sorted_item_pile_ids()
        .into_iter()
        .map(|id| {
            SceneItemPileRecord::from_record(world.item_pile_store().get(id).expect("sorted id"))
        })
        .collect();
    SceneInventoryPersistence {
        inventory_records,
        item_instance_records,
        item_instance_locations,
        corpse_records,
        item_pile_records,
        next_inventory_id: world.inventory_store().next_id(),
        next_item_instance_id: world.item_instance_store().next_id(),
        next_corpse_id: world.corpse_store().next_id(),
        next_item_pile_id: world.item_pile_store().next_id(),
    }
}

impl SceneInventoryRecord {
    pub fn from_record(record: &InventoryRecord) -> Self {
        Self {
            id: record.id().raw(),
            owner: encode_owner(record.owner()),
            profile_id: record.profile_id().as_str().to_string(),
            grid_width: record.grid_width(),
            grid_height: record.grid_height(),
            entries: record
                .placed_entries()
                .iter()
                .map(ScenePlacedEntryRecord::from_entry)
                .collect(),
        }
    }

    pub fn to_record(&self) -> Result<InventoryRecord, SceneRecordError> {
        let mut record = InventoryRecord::new(
            InventoryId::new(self.id),
            decode_owner(&self.owner)?,
            InventoryProfileId::new(&self.profile_id),
            self.grid_width,
            self.grid_height,
        );
        let entries = self
            .entries
            .iter()
            .map(ScenePlacedEntryRecord::to_entry)
            .collect::<Result<Vec<_>, _>>()?;
        *record.placed_entries_mut() = entries;
        Ok(record)
    }
}

impl ScenePlacedEntryRecord {
    pub fn from_entry(entry: &PlacedInventoryEntry) -> Self {
        match &entry.contents {
            InventoryEntryContents::Stack {
                item_definition_id,
                quantity,
            } => Self {
                anchor_x: entry.anchor_x,
                anchor_y: entry.anchor_y,
                entry_kind: "stack".into(),
                item_definition_id: Some(item_definition_id.as_str().to_string()),
                item_instance_id: None,
                quantity: Some(*quantity),
            },
            InventoryEntryContents::Unique { item_instance_id } => Self {
                anchor_x: entry.anchor_x,
                anchor_y: entry.anchor_y,
                entry_kind: "unique".into(),
                item_definition_id: None,
                item_instance_id: Some(item_instance_id.raw()),
                quantity: None,
            },
        }
    }

    pub fn to_entry(&self) -> Result<PlacedInventoryEntry, SceneRecordError> {
        Ok(match self.entry_kind.as_str() {
            "stack" => PlacedInventoryEntry::stack(
                self.anchor_x,
                self.anchor_y,
                ItemDefinitionId::new(
                    self.item_definition_id
                        .as_deref()
                        .ok_or(SceneRecordError::InvalidPosition)?,
                ),
                self.quantity.ok_or(SceneRecordError::InvalidPosition)?,
            ),
            "unique" => PlacedInventoryEntry::unique(
                self.anchor_x,
                self.anchor_y,
                ItemInstanceId::new(
                    self.item_instance_id
                        .ok_or(SceneRecordError::InvalidPosition)?,
                ),
            ),
            _ => return Err(SceneRecordError::InvalidPosition),
        })
    }
}

impl SceneItemInstanceRecord {
    pub fn from_instance(instance: &ItemInstance) -> Self {
        Self {
            id: instance.id.raw(),
            definition_id: instance.definition_id.as_str().to_string(),
            quality: instance.metadata.quality,
        }
    }

    pub fn to_instance(&self) -> ItemInstance {
        ItemInstance {
            id: ItemInstanceId::new(self.id),
            definition_id: ItemDefinitionId::new(&self.definition_id),
            metadata: ItemInstanceMetadata {
                quality: self.quality,
            },
        }
    }
}

impl SceneItemInstanceLocationRecord {
    pub fn from_location(id: ItemInstanceId, location: ItemInstanceLocation) -> Self {
        match location {
            ItemInstanceLocation::Detached => Self {
                instance_id: id.raw(),
                location_kind: "detached".into(),
                inventory_id: None,
                entry_index: None,
                pile_id: None,
            },
            ItemInstanceLocation::Inventory {
                inventory_id,
                entry_index,
            } => Self {
                instance_id: id.raw(),
                location_kind: "inventory".into(),
                inventory_id: Some(inventory_id.raw()),
                entry_index: Some(entry_index),
                pile_id: None,
            },
            ItemInstanceLocation::WorldPile(pile_id) => Self {
                instance_id: id.raw(),
                location_kind: "pile".into(),
                inventory_id: None,
                entry_index: None,
                pile_id: Some(pile_id.raw()),
            },
        }
    }

    pub fn to_location(&self) -> Result<ItemInstanceLocation, SceneRecordError> {
        Ok(match self.location_kind.as_str() {
            "detached" => ItemInstanceLocation::Detached,
            "inventory" => ItemInstanceLocation::Inventory {
                inventory_id: InventoryId::new(
                    self.inventory_id.ok_or(SceneRecordError::InvalidPosition)?,
                ),
                entry_index: self.entry_index.ok_or(SceneRecordError::InvalidPosition)?,
            },
            "pile" => ItemInstanceLocation::WorldPile(ItemPileId::new(
                self.pile_id.ok_or(SceneRecordError::InvalidPosition)?,
            )),
            _ => return Err(SceneRecordError::InvalidPosition),
        })
    }
}

impl SceneCorpseRecord {
    pub fn from_record(record: &CorpseRecord) -> Self {
        Self {
            id: record.id.raw(),
            origin_unit_id: record.origin_unit_id.raw(),
            unit_definition_id: record.unit_definition_id.as_str().to_string(),
            position: SceneWorldPosition::from_world(record.placement.position),
            rotation: SceneQuat::from_quat(record.placement.rotation),
            current_space_id: record.current_space_id.raw(),
            inventory_id: record.inventory_id.map(|id| id.raw()),
            owner_id: record.owner_id.map(|id| id.raw()),
            team_id: record.team_id.map(|id| id.raw()),
            affiliation: Some(record.affiliation.label().to_string()),
            created_tick: record.created_tick,
            remaining_lifetime_ticks: record.remaining_lifetime_ticks,
            state: match record.state {
                CorpseState::Present => "Present".into(),
                CorpseState::Expired => "Expired".into(),
            },
        }
    }

    pub fn to_record(&self) -> Result<CorpseRecord, SceneRecordError> {
        let affiliation = self
            .affiliation
            .as_deref()
            .map(affiliation_from_label)
            .unwrap_or(Affiliation::Unknown);
        let state = match self.state.as_str() {
            "Present" => CorpseState::Present,
            "Expired" => CorpseState::Expired,
            _ => return Err(SceneRecordError::InvalidPosition),
        };
        Ok(CorpseRecord {
            id: CorpseId::new(self.id),
            origin_unit_id: UnitId::new(self.origin_unit_id),
            unit_definition_id: crate::world::UnitDefinitionId::new(&self.unit_definition_id),
            placement: UnitPlacement::new(self.position.to_world()?, self.rotation.to_quat()),
            current_space_id: SpaceId::new(self.current_space_id),
            inventory_id: self.inventory_id.map(InventoryId::new),
            owner_id: self.owner_id.map(OwnerId::new),
            team_id: self.team_id.map(TeamId::new),
            affiliation,
            created_tick: self.created_tick,
            remaining_lifetime_ticks: self.remaining_lifetime_ticks,
            state,
        })
    }
}

impl SceneItemPileRecord {
    pub fn from_record(record: &WorldItemPileRecord) -> Self {
        let (contents_kind, item_definition_id, quantity, item_instance_id) = match &record.contents
        {
            WorldPileContents::Stack {
                item_definition_id,
                quantity,
            } => (
                "stack",
                Some(item_definition_id.as_str().to_string()),
                Some(*quantity),
                None,
            ),
            WorldPileContents::Unique { item_instance_id } => {
                ("unique", None, None, Some(item_instance_id.raw()))
            }
        };
        Self {
            id: record.id.raw(),
            position: SceneWorldPosition::from_world(record.placement),
            current_space_id: record.current_space_id.raw(),
            contents_kind: contents_kind.into(),
            item_definition_id,
            quantity,
            item_instance_id,
            owner_id: record.owner_id.map(|id| id.raw()),
            team_id: record.team_id.map(|id| id.raw()),
            affiliation: Some(record.affiliation.label().to_string()),
            source: pile_source_label(record.source),
            created_tick: record.created_tick,
        }
    }

    pub fn to_record(&self) -> Result<WorldItemPileRecord, SceneRecordError> {
        let affiliation = self
            .affiliation
            .as_deref()
            .map(affiliation_from_label)
            .unwrap_or(Affiliation::Unknown);
        let contents = match self.contents_kind.as_str() {
            "stack" => WorldPileContents::Stack {
                item_definition_id: ItemDefinitionId::new(
                    self.item_definition_id
                        .as_deref()
                        .ok_or(SceneRecordError::InvalidPosition)?,
                ),
                quantity: self.quantity.ok_or(SceneRecordError::InvalidPosition)?,
            },
            "unique" => WorldPileContents::Unique {
                item_instance_id: ItemInstanceId::new(
                    self.item_instance_id
                        .ok_or(SceneRecordError::InvalidPosition)?,
                ),
            },
            _ => return Err(SceneRecordError::InvalidPosition),
        };
        Ok(WorldItemPileRecord {
            id: ItemPileId::new(self.id),
            placement: self.position.to_world()?,
            current_space_id: SpaceId::new(self.current_space_id),
            contents,
            owner_id: self.owner_id.map(OwnerId::new),
            team_id: self.team_id.map(TeamId::new),
            affiliation,
            source: parse_pile_source(&self.source)?,
            created_tick: self.created_tick,
        })
    }
}

pub fn restore_inventory_persistence(
    world: &mut WorldData,
    persistence: &SceneInventoryPersistence,
    ctx: &crate::world::InventoryCatalogCtx<'_>,
) -> Result<(), String> {
    let inventory_records = persistence
        .inventory_records
        .iter()
        .map(|scene| {
            scene
                .to_record()
                .map_err(|err| format!("inventory {}: {err:?}", scene.id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    world
        .inventory_store_mut()
        .restore_snapshot(inventory_records, persistence.next_inventory_id.max(1))
        .map_err(|err| err.to_string())?;

    let instances: Vec<_> = persistence
        .item_instance_records
        .iter()
        .map(SceneItemInstanceRecord::to_instance)
        .collect();
    let locations: Vec<_> = persistence
        .item_instance_locations
        .iter()
        .map(|scene| Ok((ItemInstanceId::new(scene.instance_id), scene.to_location()?)))
        .collect::<Result<Vec<_>, SceneRecordError>>()
        .map_err(|err| format!("{err:?}"))?;
    world
        .item_instance_store_mut()
        .restore_snapshot(
            instances,
            locations,
            persistence.next_item_instance_id.max(1),
        )
        .map_err(|err| err.to_string())?;

    crate::world::rebuild_all_inventory_derived(world, ctx).map_err(|err| err.to_string())?;

    let corpse_pairs = persistence
        .corpse_records
        .iter()
        .map(|scene| {
            let record = scene
                .to_record()
                .map_err(|err| format!("corpse {}: {err:?}", scene.id))?;
            let chunk = ChunkId::new(record.placement.position.chunk);
            Ok((chunk, record))
        })
        .collect::<Result<Vec<_>, String>>()?;
    world
        .corpse_store_mut()
        .restore_snapshot(corpse_pairs, persistence.next_corpse_id.max(1))
        .map_err(|err| err.to_string())?;

    let pile_pairs = persistence
        .item_pile_records
        .iter()
        .map(|scene| {
            let record = scene
                .to_record()
                .map_err(|err| format!("pile {}: {err:?}", scene.id))?;
            let chunk = ChunkId::new(record.placement.chunk);
            Ok((chunk, record))
        })
        .collect::<Result<Vec<_>, String>>()?;
    world
        .item_pile_store_mut()
        .restore_snapshot(pile_pairs, persistence.next_item_pile_id.max(1))
        .map_err(|err| err.to_string())?;

    Ok(())
}

fn encode_owner(owner: &InventoryOwnerRef) -> String {
    match owner {
        InventoryOwnerRef::Detached => "detached".into(),
        InventoryOwnerRef::Unit(id) => format!("unit:{}", id.raw()),
        InventoryOwnerRef::Building(id) => format!("building:{}", id.raw()),
        InventoryOwnerRef::Corpse(id) => format!("corpse:{}", id.raw()),
    }
}

fn decode_owner(label: &str) -> Result<InventoryOwnerRef, SceneRecordError> {
    if label == "detached" {
        return Ok(InventoryOwnerRef::Detached);
    }
    let (kind, raw) = label
        .split_once(':')
        .ok_or(SceneRecordError::InvalidPosition)?;
    let id = raw
        .parse::<u64>()
        .map_err(|_| SceneRecordError::InvalidPosition)?;
    Ok(match kind {
        "unit" => InventoryOwnerRef::Unit(UnitId::new(id)),
        "building" => InventoryOwnerRef::Building(BuildingId::new(id)),
        "corpse" => InventoryOwnerRef::Corpse(CorpseId::new(id)),
        _ => return Err(SceneRecordError::InvalidPosition),
    })
}

fn pile_source_label(source: ItemPileSource) -> String {
    match source {
        ItemPileSource::Dropped => "Dropped".into(),
        ItemPileSource::Spilled => "Spilled".into(),
        ItemPileSource::DevSpawned => "DevSpawned".into(),
    }
}

fn parse_pile_source(label: &str) -> Result<ItemPileSource, SceneRecordError> {
    Ok(match label {
        "Dropped" => ItemPileSource::Dropped,
        "Spilled" => ItemPileSource::Spilled,
        "DevSpawned" => ItemPileSource::DevSpawned,
        _ => return Err(SceneRecordError::InvalidPosition),
    })
}
