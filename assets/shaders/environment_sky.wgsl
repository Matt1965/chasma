#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    view_transformations::position_world_to_clip,
}

struct SkyPresentationMaterial {
    horizon_color: vec4<f32>,
    zenith_color: vec4<f32>,
    twilight_color: vec4<f32>,
    twilight_strength: f32,
    sun_direction: vec3<f32>,
    sun_disc_color: vec4<f32>,
    sun_disc_intensity: f32,
    sun_cos_radius: f32,
    sun_cos_softness: f32,
    _pad0: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> material: SkyPresentationMaterial;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0),
    );
    let clip_position = position_world_to_clip(out.world_position.xyz);
    // Push sky geometry to the far plane so scene depth always wins.
    out.position = vec4<f32>(clip_position.xy, clip_position.w, clip_position.w);
    out.world_normal = normalize((world_from_local * vec4<f32>(vertex.position, 0.0)).xyz);

    return out;
}

fn sky_gradient(view_dir: vec3<f32>) -> vec3<f32> {
    let up = clamp(view_dir.y, -1.0, 1.0);
    let horizon_weight = pow(1.0 - abs(up), 2.0);
    let base_sky = mix(material.zenith_color.rgb, material.horizon_color.rgb, horizon_weight);

    let sun_alignment = saturate(dot(view_dir, normalize(material.sun_direction)));
    let localized = pow(sun_alignment, 8.0) * horizon_weight * material.twilight_strength;
    let localized_weight = clamp(localized, 0.0, 1.0);

    return mix(base_sky, material.twilight_color.rgb, localized_weight);
}

fn sun_disc(view_dir: vec3<f32>) -> vec3<f32> {
    if material.sun_disc_intensity <= 0.0 {
        return vec3<f32>(0.0);
    }

    let sun_dir = normalize(material.sun_direction);
    let cos_angle = dot(view_dir, sun_dir);
    let disc = smoothstep(
        material.sun_cos_radius - material.sun_cos_softness,
        material.sun_cos_radius + material.sun_cos_softness,
        cos_angle,
    );
    let halo = pow(saturate(cos_angle), 32.0) * 0.35;
    return material.sun_disc_color.rgb * material.sun_disc_intensity * (disc + halo);
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    let view_dir = normalize(in.world_normal);
    let rgb = sky_gradient(view_dir) + sun_disc(view_dir);
    out.color = vec4<f32>(rgb, 1.0);
    return out;
}
