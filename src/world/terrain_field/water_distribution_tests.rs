//! Full-world Water field distribution analysis (generator tuning regression).

#[cfg(test)]
mod tests {
    use std::collections::{HashSet, VecDeque};
    use std::path::Path;

    use crate::terrain::catalog::TerrainWorldCatalog;
    use crate::world::{
        BuildDependencies, TerrainFieldId, TerrainFieldSourceProfileCatalog, WorldConfig,
        build_and_package_field, build_field_layer_from_profile, starter_source_profiles,
    };

    const RICH_U16: u16 = (0.60 * 65_535.0) as u16;
    const VERY_RICH_U16: u16 = (0.80 * 65_535.0) as u16;

    struct WaterDistribution {
        sample_count: u64,
        zero: u64,
        pct_1_10: u64,
        pct_10_30: u64,
        pct_30_60: u64,
        pct_60_80: u64,
        pct_80_100: u64,
        min: u16,
        max: u16,
        mean: f64,
        p50: u16,
        p75: u16,
        p90: u16,
        p95: u16,
        p99: u16,
        rich_component_count: usize,
        rich_largest_component_cells: usize,
        very_rich_component_count: usize,
        very_rich_largest_component_cells: usize,
    }

    fn main_world_setup() -> (
        crate::world::ChunkExtent,
        WorldConfig,
        TerrainFieldSourceProfileCatalog,
        BuildDependencies<'static>,
    ) {
        let config = WorldConfig::default();
        let catalog = TerrainWorldCatalog::from_manifest(
            Path::new("assets/worlds/main/manifest.ron"),
            &config,
        )
        .expect("main terrain manifest");
        let extent = catalog.authored_extent();
        let source_catalog =
            TerrainFieldSourceProfileCatalog::from_profiles(starter_source_profiles()).unwrap();
        let deps = BuildDependencies {
            terrain_manifest_path: Some(Path::new("assets/worlds/main/manifest.ron")),
            ..Default::default()
        };
        (extent, config, source_catalog, deps)
    }

    fn pct_from_u16(value: u16) -> f64 {
        value as f64 / 65_535.0 * 100.0
    }

    fn bucket(value: u16) -> &'static str {
        let pct = pct_from_u16(value);
        if value == 0 {
            "zero"
        } else if pct < 10.0 {
            "1-10"
        } else if pct < 30.0 {
            "10-30"
        } else if pct < 60.0 {
            "30-60"
        } else if pct < 80.0 {
            "60-80"
        } else {
            "80-100"
        }
    }

    fn analyze_layer(
        layer: &crate::world::terrain_field::layer::TerrainFieldLayer,
    ) -> WaterDistribution {
        let mut values = Vec::new();
        let mut grid: Vec<(i32, i32, u16)> = Vec::new();
        let edge = crate::world::terrain_field::contract::TERRAIN_FIELD_SAMPLES_PER_EDGE as i32;

        for (chunk, tile) in &layer.tiles {
            for row in 0..edge {
                for col in 0..edge {
                    let idx = (row * edge + col) as usize;
                    let value = tile.samples[idx];
                    values.push(value);
                    let gx = chunk.x * edge + col;
                    let gz = chunk.z * edge + row;
                    grid.push((gx, gz, value));
                }
            }
        }

        values.sort_unstable();
        let sample_count = values.len() as u64;
        let mut hist = [0u64; 6];
        for &value in &values {
            match bucket(value) {
                "zero" => hist[0] += 1,
                "1-10" => hist[1] += 1,
                "10-30" => hist[2] += 1,
                "30-60" => hist[3] += 1,
                "60-80" => hist[4] += 1,
                _ => hist[5] += 1,
            }
        }

        let percentile = |p: f64| -> u16 {
            let idx = ((sample_count as f64 - 1.0) * p).round() as usize;
            values[idx.min(values.len() - 1)]
        };

        let mean = values.iter().map(|v| *v as u64).sum::<u64>() as f64 / sample_count as f64;
        let (rich_count, rich_largest) = largest_components(&grid, RICH_U16, 4);
        let (very_rich_count, very_rich_largest) = largest_components(&grid, VERY_RICH_U16, 4);

        WaterDistribution {
            sample_count,
            zero: hist[0],
            pct_1_10: hist[1],
            pct_10_30: hist[2],
            pct_30_60: hist[3],
            pct_60_80: hist[4],
            pct_80_100: hist[5],
            min: *values.first().unwrap_or(&0),
            max: *values.last().unwrap_or(&0),
            mean,
            p50: percentile(0.50),
            p75: percentile(0.75),
            p90: percentile(0.90),
            p95: percentile(0.95),
            p99: percentile(0.99),
            rich_component_count: rich_count,
            rich_largest_component_cells: rich_largest,
            very_rich_component_count: very_rich_count,
            very_rich_largest_component_cells: very_rich_largest,
        }
    }

    fn largest_components(grid: &[(i32, i32, u16)], threshold: u16, stride: i32) -> (usize, usize) {
        let mut cells = HashSet::new();
        for &(x, z, value) in grid {
            if value >= threshold && x % stride == 0 && z % stride == 0 {
                cells.insert((x / stride, z / stride));
            }
        }
        let mut visited = HashSet::new();
        let mut largest = 0usize;
        let mut count = 0usize;
        for &start in &cells {
            if visited.contains(&start) {
                continue;
            }
            count += 1;
            let mut queue = VecDeque::from([start]);
            let mut size = 0usize;
            visited.insert(start);
            while let Some((x, z)) = queue.pop_front() {
                size += 1;
                for (nx, nz) in [(x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)] {
                    let key = (nx, nz);
                    if cells.contains(&key) && visited.insert(key) {
                        queue.push_back(key);
                    }
                }
            }
            largest = largest.max(size);
        }
        (count, largest)
    }

    fn print_distribution(label: &str, dist: &WaterDistribution) {
        let pct = |n: u64| n as f64 / dist.sample_count as f64 * 100.0;
        eprintln!("=== {label} ===");
        eprintln!("samples: {}", dist.sample_count);
        eprintln!("zero: {:.2}%", pct(dist.zero));
        eprintln!("1-10%: {:.2}%", pct(dist.pct_1_10));
        eprintln!("10-30%: {:.2}%", pct(dist.pct_10_30));
        eprintln!("30-60%: {:.2}%", pct(dist.pct_30_60));
        eprintln!("60-80%: {:.2}%", pct(dist.pct_60_80));
        eprintln!("80-100%: {:.2}%", pct(dist.pct_80_100));
        eprintln!(
            "min: {:.2}% max: {:.2}% mean: {:.2}%",
            pct_from_u16(dist.min),
            pct_from_u16(dist.max),
            dist.mean / 65_535.0 * 100.0
        );
        eprintln!(
            "P50 {:.2}% P75 {:.2}% P90 {:.2}% P95 {:.2}% P99 {:.2}%",
            pct_from_u16(dist.p50),
            pct_from_u16(dist.p75),
            pct_from_u16(dist.p90),
            pct_from_u16(dist.p95),
            pct_from_u16(dist.p99)
        );
        eprintln!(
            "rich (>=60%) components: {} largest: {} cells (~{}m)",
            dist.rich_component_count,
            dist.rich_largest_component_cells,
            dist.rich_largest_component_cells * 32
        );
        eprintln!(
            "very rich (>=80%) components: {} largest: {} cells (~{}m)",
            dist.very_rich_component_count,
            dist.very_rich_largest_component_cells,
            dist.very_rich_largest_component_cells * 32
        );
    }

    #[test]
    fn water_field_full_world_distribution() {
        let (extent, config, source_catalog, deps) = main_world_setup();
        let profile = source_catalog
            .for_field(&TerrainFieldId::new("water"))
            .expect("water profile");
        let (layer, _) =
            build_field_layer_from_profile(profile, extent, &config, &deps).expect("build water");
        let dist = analyze_layer(&layer);
        print_distribution("main world water", &dist);

        assert!(pct_from_u16(dist.max) > 60.0, "max should reach rich tier");
        assert!(dist.pct_30_60 > 0, "should have good-tier samples");
        assert!(
            dist.pct_60_80 + dist.pct_80_100 > 0,
            "should have rich+ samples"
        );
        assert!(
            dist.rich_largest_component_cells >= 16,
            "rich areas should form regions, not isolated spikes"
        );
        assert!(
            dist.zero as f64 / dist.sample_count as f64 > 0.30,
            "substantial dry terrain"
        );
    }

    #[test]
    #[ignore = "writes baked water tiles to assets/worlds/main/terrain_fields"]
    fn bake_main_world_water_field() {
        let (extent, config, source_catalog, deps) = main_world_setup();
        let profile = source_catalog
            .for_field(&TerrainFieldId::new("water"))
            .expect("water profile");
        let output = Path::new("assets/worlds/main/terrain_fields");
        let (report, _) = build_and_package_field(profile, extent, &config, output, "main", &deps)
            .expect("package water");
        let dist = WaterDistribution {
            sample_count: report.statistics.sample_count,
            zero: report.statistics.zero_count,
            pct_1_10: 0,
            pct_10_30: 0,
            pct_30_60: 0,
            pct_60_80: 0,
            pct_80_100: 0,
            min: report.statistics.minimum,
            max: report.statistics.maximum,
            mean: report.statistics.average as f64,
            p50: 0,
            p75: 0,
            p90: 0,
            p95: 0,
            p99: 0,
            rich_component_count: 0,
            rich_largest_component_cells: 0,
            very_rich_component_count: 0,
            very_rich_largest_component_cells: 0,
        };
        eprintln!(
            "packaged water: tiles={} version={} min={} max={} avg={:.0}",
            report.tile_count,
            report.source_version,
            report.statistics.minimum,
            report.statistics.maximum,
            report.statistics.average
        );
        eprintln!(
            "zero% basis points: {}",
            report.statistics.zero_percent_basis_points()
        );
        let _ = dist;
    }
}
