use crate::world::{ChunkId, ChunkLayout};

/// Inputs for procedural doodad generation (ADR-018).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DoodadGenerationContext<'a> {
    pub world_seed: u64,
    pub chunk: ChunkId,
    pub layout: &'a ChunkLayout,
}

impl<'a> DoodadGenerationContext<'a> {
    pub fn new(world_seed: u64, chunk: ChunkId, layout: &'a ChunkLayout) -> Self {
        Self {
            world_seed,
            chunk,
            layout,
        }
    }
}
