use bevy::prelude::*;

/// How a building occupies horizontal space (B1).
///
/// Describes footprint shape only — occupancy baking and navigation integration
/// are deferred to later phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum FootprintType {
    #[default]
    Rectangle,
    Circle,
    /// Occupancy derived offline from [`crate::world::BuildingDefinition::collision_render_key`].
    MeshDerived,
}

impl FootprintType {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "rectangle" | "rect" => Ok(Self::Rectangle),
            "circle" => Ok(Self::Circle),
            "meshderived" | "mesh derived" | "mesh_derived" | "mesh" => Ok(Self::MeshDerived),
            other => Err(format!("unknown Footprint Type `{other}`")),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangle => "Rectangle",
            Self::Circle => "Circle",
            Self::MeshDerived => "MeshDerived",
        }
    }
}

/// Authoritative footprint dimensions for simple shapes (B1).
///
/// [`FootprintType::MeshDerived`] definitions use [`FootprintSpec::MeshDerived`] and
/// rely on a collision mesh reference for future offline baking.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub enum FootprintSpec {
    Rectangle {
        width_meters: f32,
        depth_meters: f32,
    },
    Circle {
        radius_meters: f32,
    },
    MeshDerived,
}

impl FootprintSpec {
    pub fn footprint_type(&self) -> FootprintType {
        match self {
            Self::Rectangle { .. } => FootprintType::Rectangle,
            Self::Circle { .. } => FootprintType::Circle,
            Self::MeshDerived => FootprintType::MeshDerived,
        }
    }
}
