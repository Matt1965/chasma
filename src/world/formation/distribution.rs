//! Offset point generation for formation layouts (ADR-035 U10).

use super::layout::FormationKind;
use super::offsets::FormationOffset;

/// Ring radius so adjacent slots are at least `spacing` meters apart.
pub fn circle_formation_radius(unit_count: usize, spacing: f32) -> f32 {
    if unit_count <= 1 {
        return 0.0;
    }
    let n = unit_count as f32;
    let half_angle = std::f32::consts::PI / n;
    let sin = half_angle.sin().max(1e-4);
    (spacing / (2.0 * sin)).max(spacing * 0.5)
}

/// Evenly spaced slots on a horizontal ring.
pub fn circle_offsets(unit_count: usize, spacing: f32) -> Vec<FormationOffset> {
    if unit_count == 0 {
        return Vec::new();
    }
    if unit_count == 1 {
        return vec![FormationOffset::ZERO];
    }

    let radius = circle_formation_radius(unit_count, spacing);
    (0..unit_count)
        .map(|index| {
            let angle = std::f32::consts::TAU * index as f32 / unit_count as f32;
            FormationOffset::new(angle.cos() * radius, angle.sin() * radius)
        })
        .collect()
}

/// Evenly spaced slots on a horizontal line through the center (global X axis).
pub fn line_offsets(unit_count: usize, spacing: f32) -> Vec<FormationOffset> {
    if unit_count == 0 {
        return Vec::new();
    }
    if unit_count == 1 {
        return vec![FormationOffset::ZERO];
    }

    let total_span = spacing * (unit_count - 1) as f32;
    let start = -total_span * 0.5;
    (0..unit_count)
        .map(|index| {
            let x = start + index as f32 * spacing;
            FormationOffset::new(x, 0.0)
        })
        .collect()
}

/// Row/column block centered on the click point.
pub fn grid_offsets(unit_count: usize, spacing: f32) -> Vec<FormationOffset> {
    if unit_count == 0 {
        return Vec::new();
    }
    if unit_count == 1 {
        return vec![FormationOffset::ZERO];
    }

    let cols = (unit_count as f32).sqrt().ceil() as usize;
    let rows = unit_count.div_ceil(cols);
    let col_span = (cols.saturating_sub(1) as f32) * spacing;
    let row_span = (rows.saturating_sub(1) as f32) * spacing;

    (0..unit_count)
        .map(|index| {
            let row = index / cols;
            let col = index % cols;
            let x = col as f32 * spacing - col_span * 0.5;
            let z = row as f32 * spacing - row_span * 0.5;
            FormationOffset::new(x, z)
        })
        .collect()
}

pub fn formation_offsets(
    kind: FormationKind,
    unit_count: usize,
    spacing: f32,
) -> Vec<FormationOffset> {
    match kind {
        FormationKind::Grid => grid_offsets(unit_count, spacing),
        FormationKind::Line => line_offsets(unit_count, spacing),
        FormationKind::Circle => circle_offsets(unit_count, spacing),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn different_unit_counts_change_circle_radius() {
        let small = circle_formation_radius(3, 1.2);
        let large = circle_formation_radius(12, 1.2);
        assert!(large > small);
    }

    #[test]
    fn circle_offsets_spread_multiple_units() {
        let offsets = circle_offsets(4, 1.2);
        assert_eq!(offsets.len(), 4);
        assert!(offsets.iter().any(|offset| offset.xz.length() > 0.5));
    }

    #[test]
    fn line_offsets_are_symmetric() {
        let offsets = line_offsets(3, 2.0);
        assert_eq!(offsets.len(), 3);
        assert!((offsets[0].xz.x + offsets[2].xz.x).abs() < 1e-4);
        assert!(offsets[1].xz.x.abs() < 1e-4);
    }

    #[test]
    fn grid_offsets_form_centered_rows() {
        let offsets = grid_offsets(100, 1.2);
        assert_eq!(offsets.len(), 100);
        let avg_x = offsets.iter().map(|o| o.xz.x).sum::<f32>() / 100.0;
        let avg_z = offsets.iter().map(|o| o.xz.y).sum::<f32>() / 100.0;
        assert!(avg_x.abs() < 1e-3);
        assert!(avg_z.abs() < 1e-3);
    }
}
