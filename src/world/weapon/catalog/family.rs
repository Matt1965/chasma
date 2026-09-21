use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Weapon animation classification for presentation (Slice 8).
///
/// Gameplay remains on [`super::definition::WeaponDefinition`]; this groups
/// attack/combat-idle clips without item-id special cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default, Serialize, Deserialize)]
pub enum WeaponAnimationFamily {
    #[default]
    None,
    Unarmed,
    OneHandSword,
}

impl WeaponAnimationFamily {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().replace(' ', "").as_str() {
            "" | "none" => Ok(Self::None),
            "unarmed" | "fists" => Ok(Self::Unarmed),
            "onehandsword" | "one_hand_sword" | "one-hand-sword" | "sword" => Ok(Self::OneHandSword),
            other => Err(format!("unknown Weapon Animation Family `{other}`")),
        }
    }
}
