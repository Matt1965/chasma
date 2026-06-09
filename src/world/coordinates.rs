use bevy::prelude::*;

/// Integer chunk coordinate on the 2D horizontal grid.
///
/// The chunk grid tiles the XZ plane; the vertical (Y) axis is not chunked
/// (ADR-006). A coordinate addresses a square region of the world: chunk
/// `(x, z)` covers world X in `[x * size, (x + 1) * size)` and world Z in
/// `[z * size, (z + 1) * size)`, where `size` is the chunk size in world units
/// (ADR-001 addendum).
///
/// The chunk coordinate is also the authoritative chunk identity (see
/// [`crate::world::ChunkId`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

/// A position relative to a chunk's minimum (lowest X, lowest Z) corner.
///
/// `x` and `z` lie in `[0, chunk_size)` in world units. `y` is absolute terrain
/// height, because the vertical axis is not chunked (ADR-001 addendum, ADR-006).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct LocalPosition(pub Vec3);

impl LocalPosition {
    pub const fn new(value: Vec3) -> Self {
        Self(value)
    }
}

/// The authoritative world position: a chunk coordinate plus a chunk-relative
/// local position (ADR-001).
///
/// World data, simulation, persistence, and queries use this representation as
/// the source of truth. Rendering derives a global [`Vec3`] from it on demand
/// via [`WorldPosition::to_global`].
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct WorldPosition {
    pub chunk: ChunkCoord,
    pub local: LocalPosition,
}

/// The minimal, copyable description of the chunk grid that coordinate
/// conversions need.
///
/// Conversions take a `ChunkLayout` rather than baking in a constant chunk size,
/// so chunk size remains owned by [`crate::world::WorldConfig`] as the single
/// authoritative source (ADR-002 addendum).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct ChunkLayout {
    /// Chunk edge length in meters (ADR-002 default: 256).
    pub chunk_size_meters: f32,
    /// World units per meter (ADR-001 addendum: 1 unit = 1 meter).
    pub units_per_meter: f32,
}

impl ChunkLayout {
    /// Chunk edge length expressed in world units.
    pub fn chunk_size_units(self) -> f32 {
        self.chunk_size_meters * self.units_per_meter
    }
}

impl WorldPosition {
    pub const fn new(chunk: ChunkCoord, local: LocalPosition) -> Self {
        Self { chunk, local }
    }

    /// Decompose a global render-space position into the authoritative
    /// chunk-relative representation.
    pub fn from_global(global: Vec3, layout: ChunkLayout) -> Self {
        let size = layout.chunk_size_units();
        let cx = (global.x / size).floor() as i32;
        let cz = (global.z / size).floor() as i32;
        let local = Vec3::new(
            global.x - cx as f32 * size,
            global.y,
            global.z - cz as f32 * size,
        );
        Self {
            chunk: ChunkCoord::new(cx, cz),
            local: LocalPosition::new(local),
        }
    }

    /// Compose a global render-space position from the authoritative
    /// chunk-relative representation.
    pub fn to_global(self, layout: ChunkLayout) -> Vec3 {
        let size = layout.chunk_size_units();
        Vec3::new(
            self.chunk.x as f32 * size + self.local.0.x,
            self.local.0.y,
            self.chunk.z as f32 * size + self.local.0.z,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn assert_vec3_eq(a: Vec3, b: Vec3) {
        assert!((a - b).length() < 1e-3, "{a:?} != {b:?}");
    }

    #[test]
    fn composes_global_from_chunk_relative() {
        let pos = WorldPosition::new(
            ChunkCoord::new(1, 2),
            LocalPosition::new(Vec3::new(10.0, 5.0, 20.0)),
        );
        assert_vec3_eq(pos.to_global(layout()), Vec3::new(266.0, 5.0, 532.0));
    }

    #[test]
    fn decomposes_global_into_chunk_relative() {
        let pos = WorldPosition::from_global(Vec3::new(266.0, 5.0, 532.0), layout());
        assert_eq!(pos.chunk, ChunkCoord::new(1, 2));
        assert_vec3_eq(pos.local.0, Vec3::new(10.0, 5.0, 20.0));
    }

    #[test]
    fn handles_negative_coordinates() {
        let pos = WorldPosition::from_global(Vec3::new(-1.0, 0.0, -1.0), layout());
        assert_eq!(pos.chunk, ChunkCoord::new(-1, -1));
        assert_vec3_eq(pos.local.0, Vec3::new(255.0, 0.0, 255.0));
    }

    #[test]
    fn chunk_boundary_belongs_to_higher_chunk() {
        let pos = WorldPosition::from_global(Vec3::new(256.0, 0.0, 0.0), layout());
        assert_eq!(pos.chunk, ChunkCoord::new(1, 0));
        assert_vec3_eq(pos.local.0, Vec3::ZERO);
    }

    #[test]
    fn round_trips_through_global() {
        let samples = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(123.5, 40.25, 7.75),
            Vec3::new(-513.0, -2.0, 1024.5),
            Vec3::new(255.999, 100.0, -0.001),
        ];
        for global in samples {
            let round_tripped = WorldPosition::from_global(global, layout()).to_global(layout());
            assert_vec3_eq(round_tripped, global);
        }
    }
}
