#import bevy_pbr::{
    forward_io::{FragmentOutput, VertexOutput},
    prepass_utils,
    view_transformations::depth_ndc_to_view_z,
}

struct OceanDepthMaterial {
    shallow_color: vec4<f32>,
    deep_color: vec4<f32>,
    absorption_depth: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> material: OceanDepthMaterial;

/// View-space distance from the water surface to the nearest opaque scene surface
/// along the same screen pixel (positive = scene is behind the water surface).
fn water_column_depth_view(in: VertexOutput) -> f32 {
#ifdef DEPTH_PREPASS
    let water_view_z = depth_ndc_to_view_z(in.position.z);
    let scene_ndc = prepass_utils::prepass_depth(in.position, 0u);
    let scene_view_z = depth_ndc_to_view_z(scene_ndc);
    return max(water_view_z - scene_view_z, 0.0);
#else
    // Without a prepass depth texture, treat as open/deep water.
    return material.absorption_depth * 8.0;
#endif
}

fn depth_response_factor(column_depth: f32) -> f32 {
    let scale = max(material.absorption_depth, 0.001);
    return saturate(1.0 - exp(-column_depth / scale));
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let factor = depth_response_factor(water_column_depth_view(in));
    let rgb = mix(material.shallow_color.rgb, material.deep_color.rgb, factor);
    let alpha = mix(material.shallow_color.a, material.deep_color.a, factor);

    out.color = vec4<f32>(rgb, alpha);
    return out;
}
