use exr::prelude::read_first_flat_layer_from_file;
fn main() {
    let path = "source_data/test/color/Albedo_y0_x0.exr";
    let img = read_first_flat_layer_from_file(path).expect("read");
    let layer = img.layer_data;
    println!("size {}x{}", layer.size.0, layer.size.1);
    for (i, ch) in layer.channel_data.list.iter().enumerate() {
        use exr::prelude::FlatSamples;
        let (min, max, first3): (f32, f32, Vec<f32>) = match &ch.sample_data {
            FlatSamples::F32(v) => {
                let min = v.iter().copied().fold(f32::INFINITY, f32::min);
                let max = v.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                (min, max, v.iter().take(3).copied().collect())
            }
            FlatSamples::F16(v) => {
                let min = v.iter().map(|h| h.to_f32()).fold(f32::INFINITY, f32::min);
                let max = v.iter().map(|h| h.to_f32()).fold(f32::NEG_INFINITY, f32::max);
                (min, max, v.iter().take(3).map(|h| h.to_f32()).collect())
            }
            _ => (0.0, 0.0, vec![]),
        };
        println!("ch[{i}] min={min} max={max} first3={first3:?}");
    }
}
