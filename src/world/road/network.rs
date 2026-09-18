use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::crossing::RoadCrossingOverride;
use super::id::{JunctionId, RoadId};
use super::junction::Junction;
use super::road::Road;
use super::style::{RoadStyleDefaults, RoadStyleId, default_style_table};

pub const ROAD_NETWORK_SCHEMA_VERSION: u32 = 1;

/// Runtime and durable road network authority.
#[derive(Debug, Clone, PartialEq, Resource, Reflect)]
pub struct RoadNetwork {
    pub version: u32,
    pub styles: BTreeMap<RoadStyleId, RoadStyleDefaults>,
    pub roads: BTreeMap<RoadId, Road>,
    pub junctions: BTreeMap<JunctionId, Junction>,
    pub crossings: Vec<RoadCrossingOverride>,
}

impl Default for RoadNetwork {
    fn default() -> Self {
        Self::empty()
    }
}

impl RoadNetwork {
    pub fn empty() -> Self {
        Self {
            version: ROAD_NETWORK_SCHEMA_VERSION,
            styles: default_style_table(),
            roads: BTreeMap::new(),
            junctions: BTreeMap::new(),
            crossings: Vec::new(),
        }
    }

    pub fn road(&self, id: &RoadId) -> Option<&Road> {
        self.roads.get(id)
    }

    pub fn junction(&self, id: &JunctionId) -> Option<&Junction> {
        self.junctions.get(id)
    }

    pub fn style_defaults(&self, style: RoadStyleId) -> Option<&RoadStyleDefaults> {
        self.styles.get(&style)
    }
}

/// RON-serializable road network document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoadNetworkRon {
    pub version: u32,
    #[serde(default = "default_style_table")]
    pub styles: BTreeMap<RoadStyleId, RoadStyleDefaults>,
    #[serde(default)]
    pub roads: BTreeMap<RoadId, Road>,
    #[serde(default)]
    pub junctions: BTreeMap<JunctionId, Junction>,
    #[serde(default)]
    pub crossings: Vec<RoadCrossingOverride>,
}

impl RoadNetworkRon {
    pub fn into_network(self) -> RoadNetwork {
        RoadNetwork {
            version: self.version,
            styles: self.styles,
            roads: self.roads,
            junctions: self.junctions,
            crossings: self.crossings,
        }
    }
}

impl From<RoadNetwork> for RoadNetworkRon {
    fn from(value: RoadNetwork) -> Self {
        Self {
            version: value.version,
            styles: value.styles,
            roads: value.roads,
            junctions: value.junctions,
            crossings: value.crossings,
        }
    }
}

impl From<RoadNetworkRon> for RoadNetwork {
    fn from(value: RoadNetworkRon) -> Self {
        value.into_network()
    }
}
