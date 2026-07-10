use bevy::prelude::*;

use crate::world::coordinates::WorldPosition;
use crate::world::combat::ProjectileLaunchSnapshot;
use crate::world::unit::UnitId;
use crate::world::weapon::{DamageType, WeaponDefinitionId};

use super::id::ProjectileId;

/// Lifecycle state of an authoritative projectile (ADR-060 C7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum ProjectileStatus {
    #[default]
    InFlight,
    Hit,
    Expired,
    Invalidated,
}

/// Authoritative projectile simulation record (ADR-060 C7).
///
/// Damage payload is fixed at launch. Visual ECS entities mirror this data only.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct ProjectileRecord {
    pub id: ProjectileId,
    pub source_unit_id: UnitId,
    pub target_unit_id: UnitId,
    pub weapon_id: WeaponDefinitionId,
    pub damage: f32,
    pub damage_type: DamageType,
    pub position: WorldPosition,
    pub target_position_snapshot: WorldPosition,
    pub speed_mps: f32,
    pub status: ProjectileStatus,
    /// Frozen ownership and weapon-filter context from launch (REVIEW-A3).
    #[reflect(ignore)]
    pub launch_snapshot: ProjectileLaunchSnapshot,
}

impl ProjectileRecord {
    pub fn new_in_flight(
        id: ProjectileId,
        source_unit_id: UnitId,
        target_unit_id: UnitId,
        weapon_id: WeaponDefinitionId,
        damage: f32,
        damage_type: DamageType,
        position: WorldPosition,
        target_position_snapshot: WorldPosition,
        speed_mps: f32,
        launch_snapshot: ProjectileLaunchSnapshot,
    ) -> Self {
        Self {
            id,
            source_unit_id,
            target_unit_id,
            weapon_id,
            damage,
            damage_type,
            position,
            target_position_snapshot,
            speed_mps,
            status: ProjectileStatus::InFlight,
            launch_snapshot,
        }
    }
}
