//! Placement brush modes (ADR-044).

use bevy::prelude::*;

use crate::world::{ChunkLayout, WorldPosition};

use super::pattern::{
    circle_offsets, dev_placement_seed, grid_offsets, line_offsets, offsets_to_world_positions,
    scatter_offsets, PatternPointBuffer,
};

/// How a single click expands into multiple spawn candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum BrushMode {
    #[default]
    SingleClick,
    Line,
    Circle,
    Grid,
    RandomScatter,
}

impl BrushMode {
    pub fn label(self) -> &'static str {
        match self {
            BrushMode::SingleClick => "Single",
            BrushMode::Line => "Line",
            BrushMode::Circle => "Circle",
            BrushMode::Grid => "Grid",
            BrushMode::RandomScatter => "Scatter",
        }
    }

    pub fn next(self) -> Self {
        match self {
            BrushMode::SingleClick => BrushMode::Line,
            BrushMode::Line => BrushMode::Circle,
            BrushMode::Circle => BrushMode::Grid,
            BrushMode::Grid => BrushMode::RandomScatter,
            BrushMode::RandomScatter => BrushMode::SingleClick,
        }
    }
}

/// Parameters for brush expansion (client-local authoring state).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct BrushSettings {
    pub mode: BrushMode,
    pub count: u32,
    pub spacing: f32,
    pub scatter_radius: f32,
    pub grid_columns: u32,
    pub grid_rows: u32,
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            mode: BrushMode::SingleClick,
            count: 8,
            spacing: 4.0,
            scatter_radius: 12.0,
            grid_columns: 3,
            grid_rows: 3,
        }
    }
}

/// Safety cap for one batch placement operation.
pub const MAX_BRUSH_SPAWN_COUNT: u32 = 256;

/// Reusable world-position scratch buffer for brush generation.
#[derive(Debug, Default)]
pub struct BrushPointBuffer {
    positions: Vec<WorldPosition>,
    pattern: PatternPointBuffer,
}

impl BrushPointBuffer {
    pub fn clear(&mut self) {
        self.positions.clear();
        self.pattern.clear();
    }

    pub fn positions(&self) -> &[WorldPosition] {
        &self.positions
    }
}

/// Generate world-space candidate positions for a brush click.
pub fn generate_brush_positions(
    settings: &BrushSettings,
    anchor: WorldPosition,
    layout: ChunkLayout,
    line_direction: Vec2,
    world_seed: u64,
    definition_key: &str,
    buffer: &mut BrushPointBuffer,
) {
    buffer.clear();
    let count = settings.count.min(MAX_BRUSH_SPAWN_COUNT);

    let pattern = match settings.mode {
        BrushMode::SingleClick => {
            buffer.pattern.clear();
            buffer.pattern.push_offset(Vec2::ZERO);
            &buffer.pattern
        }
        BrushMode::Line => {
            buffer.pattern = line_offsets(count, settings.spacing, line_direction);
            &buffer.pattern
        }
        BrushMode::Circle => {
            buffer.pattern = circle_offsets(count, settings.scatter_radius);
            &buffer.pattern
        }
        BrushMode::Grid => {
            buffer.pattern = grid_offsets(settings.grid_columns, settings.grid_rows, settings.spacing);
            &buffer.pattern
        }
        BrushMode::RandomScatter => {
            let seed = dev_placement_seed(world_seed, anchor, definition_key);
            buffer.pattern = scatter_offsets(count, settings.scatter_radius, seed);
            &buffer.pattern
        }
    };

    offsets_to_world_positions(anchor, layout, pattern.offsets(), &mut buffer.positions);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{ChunkCoord, LocalPosition};

    fn anchor() -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(20.0, 0.0, 20.0)),
        )
    }

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    #[test]
    fn circle_brush_generates_deterministic_points() {
        let settings = BrushSettings {
            mode: BrushMode::Circle,
            count: 6,
            scatter_radius: 8.0,
            ..Default::default()
        };
        let mut buffer = BrushPointBuffer::default();
        generate_brush_positions(
            &settings,
            anchor(),
            layout(),
            Vec2::X,
            42,
            "wolf",
            &mut buffer,
        );
        let first = buffer.positions().to_vec();
        buffer.clear();
        generate_brush_positions(
            &settings,
            anchor(),
            layout(),
            Vec2::X,
            42,
            "wolf",
            &mut buffer,
        );
        assert_eq!(first, buffer.positions());
        assert_eq!(buffer.positions().len(), 6);
    }
}
