//! Logistics runtime enums (EP7).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Hauling request priority (EP7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, Default)]
pub enum HaulingRequestPriority {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Low = 3,
}

impl HaulingRequestPriority {
    pub fn rank(self) -> u8 {
        self as u8
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Critical => "Critical",
            Self::High => "High",
            Self::Normal => "Normal",
            Self::Low => "Low",
        }
    }
}

/// Hauling request lifecycle (EP7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, Default)]
pub enum HaulingRequestStatus {
    #[default]
    Pending,
    Assigned,
    InProgress,
    PartiallyFulfilled,
    Blocked,
    Completed,
    Cancelled,
}

impl HaulingRequestStatus {
    pub fn is_open(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::Assigned | Self::InProgress | Self::PartiallyFulfilled | Self::Blocked
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Assigned => "Assigned",
            Self::InProgress => "InProgress",
            Self::PartiallyFulfilled => "PartiallyFulfilled",
            Self::Blocked => "Blocked",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
        }
    }
}

/// Why a hauling request was generated (EP7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum HaulingGenerationReason {
    OutputSurplus,
    InputDeficit,
    ManualDev,
}

impl HaulingGenerationReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::OutputSurplus => "OutputSurplus",
            Self::InputDeficit => "InputDeficit",
            Self::ManualDev => "ManualDev",
        }
    }
}

/// Reservation state on a hauling request (EP7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, Default)]
pub enum HaulingReservationState {
    #[default]
    None,
    DestinationReserved,
    SourceReserved,
    FullyReserved,
}

/// Worker execution phase for one haul (EP7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize, Default)]
pub enum HaulExecutionPhase {
    #[default]
    Pending,
    TravelingToSource,
    PickingUp,
    TravelingToDestination,
    Depositing,
    Completed,
    Failed,
}

impl HaulExecutionPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::TravelingToSource => "TravelingToSource",
            Self::PickingUp => "PickingUp",
            Self::TravelingToDestination => "TravelingToDestination",
            Self::Depositing => "Depositing",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
        }
    }
}

/// Blocking reasons for hauling requests (EP7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum HaulingBlockingReason {
    MissingSource,
    DestinationFull,
    ReservationFailed,
    BuildingRemoved,
    InventoryRemoved,
    SourceEqualsDestination,
    NoAvailableItems,
    WorkerUnavailable,
}

impl HaulingBlockingReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::MissingSource => "MissingSource",
            Self::DestinationFull => "DestinationFull",
            Self::ReservationFailed => "ReservationFailed",
            Self::BuildingRemoved => "BuildingRemoved",
            Self::InventoryRemoved => "InventoryRemoved",
            Self::SourceEqualsDestination => "SourceEqualsDestination",
            Self::NoAvailableItems => "NoAvailableItems",
            Self::WorkerUnavailable => "WorkerUnavailable",
        }
    }
}

/// Data-driven logistics route trigger (EP7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum LogisticsRouteTrigger {
    OutputSurplus,
    InputDeficit,
}
