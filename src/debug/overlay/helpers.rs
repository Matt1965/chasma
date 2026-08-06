//! Shared helpers for debug overlay rendering.

use bevy::prelude::*;

use crate::terrain::world_position_to_render_global;
use crate::world::{ChunkLayout, WorldPosition};

pub fn render_position(position: WorldPosition, layout: ChunkLayout, vertical_scale: f32) -> Vec3 {
    world_position_to_render_global(position, layout, vertical_scale)
}

pub fn xz_to_render_y(base: Vec3, y_offset: f32) -> Vec3 {
    Vec3::new(base.x, base.y + y_offset, base.z)
}

/// Closed-polygon boundary segment index pairs: `(i, (i + 1) % n)` for each vertex.
///
/// Returns exactly `n` segments for `n >= 2`. Does not emit diagonals or chords.
pub fn closed_polygon_boundary_segments(vertex_count: usize) -> Vec<(usize, usize)> {
    if vertex_count < 2 {
        return Vec::new();
    }
    (0..vertex_count)
        .map(|i| (i, (i + 1) % vertex_count))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::closed_polygon_boundary_segments;

    #[test]
    fn triangle_produces_three_consecutive_boundary_segments() {
        let segments = closed_polygon_boundary_segments(3);
        assert_eq!(segments, vec![(0, 1), (1, 2), (2, 0)]);
        assert!(segments.iter().all(|&(a, b)| (a + 1) % 3 == b));
        assert!(!segments.iter().any(|&(a, b)| (a + 2) % 3 == b && a != b));
    }

    #[test]
    fn quadrilateral_produces_four_consecutive_boundary_segments() {
        let segments = closed_polygon_boundary_segments(4);
        assert_eq!(segments.len(), 4);
        assert_eq!(segments, vec![(0, 1), (1, 2), (2, 3), (3, 0)]);
        assert!(!segments.iter().any(|&(a, b)| (a + 2) % 4 == b));
    }

    #[test]
    fn n_gon_produces_exactly_n_consecutive_boundary_segments() {
        for n in [2usize, 5, 8, 12] {
            let segments = closed_polygon_boundary_segments(n);
            assert_eq!(segments.len(), n);
            for (i, &(a, b)) in segments.iter().enumerate() {
                assert_eq!(a, i);
                assert_eq!(b, (i + 1) % n);
            }
            assert!(
                !segments.iter().any(|&(a, b)| b == (a + 2) % n && n > 2),
                "n={n} must not include i→i+2 chords"
            );
        }
    }

    #[test]
    fn fewer_than_two_vertices_yields_no_segments() {
        assert!(closed_polygon_boundary_segments(0).is_empty());
        assert!(closed_polygon_boundary_segments(1).is_empty());
    }
}
