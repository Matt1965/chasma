#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    mesh_view_bindings as view_bindings,
    prepass_utils,
    utils::interleaved_gradient_noise,
    view_transformations::{frag_coord_to_ndc, position_ndc_to_world, position_world_to_clip},
}

struct CloudLayerMaterial {
    wind_offset: vec2<f32>,
    wind_direction: vec2<f32>,
    coverage: f32,
    macro_scale: f32,
    vertical_development: f32,
    density_scale: f32,
    edge_breakup: f32,
    anisotropy: f32,
    layer_night_factor: f32,
    effective_daylight: f32,
    twilight_factor: f32,
    sun_direction: vec3<f32>,
    _pad0: f32,
    sun_color: vec4<f32>,
    low_y_min: f32,
    low_y_max: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> material: CloudLayerMaterial;

const CLOUD_MARCH_MAX_STEPS: i32 = 24;
const CLOUD_MARCH_MAX_STEPS_CAP: i32 = 32;
const CLOUD_MARCH_MIN_STEP: f32 = 60.0;
const CLOUD_MARCH_MAX_STEP: f32 = 400.0;
// Maximum world-space distance integrated along the ray AFTER entering the cloud band.
const CLOUD_MARCH_MAX_SEGMENT: f32 = 40000.0;
const BAND_RAY_EPSILON: f32 = 0.00001;
const SCENE_DEPTH_SKY_EPSILON: f32 = 0.00001;
const SCENE_DEPTH_UNBOUNDED: f32 = 1000000000.0;
const TRANSMITTANCE_CUTOFF: f32 = 0.01;
// Artistic internal: maps unit density + density_scale to 1/m extinction (CLOUD-VOL-V2A).
const EXTINCTION_ARTIFACT: f32 = 0.012;
// Artistic internals: noise scale ratios relative to macro_scale (CLOUD-VOL-V2B / V1 Step 3).
const WEATHER_MAP_RATIO: f32 = 0.35;
const BODY_NOISE_RATIO: f32 = 1.65;
const BODY_Y_ASPECT: f32 = 0.55;
// L3 erosion: ~180 m cells at default macro_scale (CLOUD-VOL-V2C / V1 Step 4).
const DETAIL_NOISE_RATIO: f32 = 11.12;
const BODY_FBM_PERSISTENCE: f32 = 0.52;
const EROSION_FBM_OCTAVES: i32 = 2;
const EROSION_HEIGHT_BIAS: f32 = 2.0;

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash31(p: vec3<f32>) -> f32 {
    var p3 = fract(p * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash31(i);
    let b = hash31(i + vec3(1.0, 0.0, 0.0));
    let c = hash31(i + vec3(0.0, 1.0, 0.0));
    let d = hash31(i + vec3(1.0, 1.0, 0.0));
    let e = hash31(i + vec3(0.0, 0.0, 1.0));
    let f0 = hash31(i + vec3(1.0, 0.0, 1.0));
    let g = hash31(i + vec3(0.0, 1.0, 1.0));
    let h = hash31(i + vec3(1.0, 1.0, 1.0));

    let x0 = mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
    let x1 = mix(mix(e, f0, u.x), mix(g, h, u.x), u.y);
    return mix(x0, x1, u.z);
}

fn fbm2(p: vec2<f32>, octaves: i32, persistence: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var total = 0.0;
    for (var i = 0; i < octaves; i = i + 1) {
        value += amplitude * noise2(p * frequency);
        total += amplitude;
        frequency *= 2.0;
        amplitude *= persistence;
    }
    return value / max(total, 0.0001);
}

fn fbm3(p: vec3<f32>, octaves: i32, persistence: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var total = 0.0;
    for (var i = 0; i < octaves; i = i + 1) {
        value += amplitude * noise3(p * frequency);
        total += amplitude;
        frequency *= 2.0;
        amplitude *= persistence;
    }
    return value / max(total, 0.0001);
}

fn remap(v: f32, l0: f32, h0: f32, l1: f32, h1: f32) -> f32 {
    return l1 + (v - l0) * (h1 - l1) / max(h0 - l0, 0.00001);
}

// Future weather-field replacement point: macro presence and stratiform/cumuliform mix (V1 L1).
fn weather_map(world_xz: vec2<f32>) -> vec2<f32> {
    let sample_xz = world_xz + material.wind_offset;
    let weather_uv = sample_xz * (material.macro_scale * WEATHER_MAP_RATIO);
    let coverage_field = fbm2(weather_uv, 2, 0.5);
    let type_field = fbm2(weather_uv * 1.7 + vec2(31.0, 17.0), 2, 0.5);
    return vec2(coverage_field, type_field);
}

fn height_profile(height01: f32, type_field: f32) -> f32 {
    let strat = smoothstep(0.05, 0.25, height01) * (1.0 - smoothstep(0.75, 0.95, height01));
    let cumulus_bottom = smoothstep(0.0, 0.12, height01);
    let cumulus_bulge = 1.0 - smoothstep(0.35, 0.65, abs(height01 - 0.45));
    let cumulus_top = 1.0 - smoothstep(0.82, 1.0, height01);
    let cumulus = cumulus_bottom * max(cumulus_bulge, 0.35) * cumulus_top;
    return mix(strat, cumulus, saturate(type_field));
}

// L2 body mass before erosion (L1 weather map + 3D body noise + height profile).
fn low_cloud_body_shape(world_pos: vec3<f32>) -> vec2<f32> {
    // Cloud density coordinates are world-space. Camera motion must never be incorporated here.
    let span = max(material.low_y_max - material.low_y_min, 0.001);
    let height01 = saturate((world_pos.y - material.low_y_min) / span);
    let weather = weather_map(world_pos.xz);
    let profile = height_profile(height01, weather.y) * material.vertical_development;

    let wind_xz = world_pos.xz + material.wind_offset;
    let body_scale = material.macro_scale * BODY_NOISE_RATIO;
    let body_p = vec3(
        wind_xz.x * body_scale,
        world_pos.y * body_scale * BODY_Y_ASPECT,
        wind_xz.y * body_scale,
    );
    // Body noise owns coherent cloud mass; persistence is an artistic internal, not edge_breakup.
    let base = fbm3(body_p, 3, BODY_FBM_PERSISTENCE);

    let coverage_threshold = 1.0 - weather.x * material.coverage;
    let shape = saturate(remap(base, coverage_threshold, 1.0, 0.0, 1.0)) * profile;
    return vec2(shape, height01);
}

// L3 erosion field: subtractive breakup biased toward cloud tops (wispy) vs bottoms (billowy).
fn erosion_field(world_pos: vec3<f32>, height01: f32) -> f32 {
    let wind_xz = world_pos.xz + material.wind_offset;
    let detail_scale = material.macro_scale * DETAIL_NOISE_RATIO;
    let detail_p = vec3(
        wind_xz.x * detail_scale,
        world_pos.y * detail_scale * BODY_Y_ASPECT,
        wind_xz.y * detail_scale,
    );
    let detail = fbm3(detail_p, EROSION_FBM_OCTAVES, 0.5);
    return mix(detail, 1.0 - detail, saturate(height01 * EROSION_HEIGHT_BIAS));
}

fn low_volumetric_density(world_pos: vec3<f32>) -> f32 {
    let body = low_cloud_body_shape(world_pos);
    if body.x <= 0.0 {
        return 0.0;
    }
    // Erosion noise primarily removes density near cloud boundaries; edge_breakup scales strength.
    let erosion = erosion_field(world_pos, body.y);
    return saturate(remap(body.x, erosion * material.edge_breakup, 1.0, 0.0, 1.0));
}

fn cloud_sample_color(view_dir: vec3<f32>) -> vec3<f32> {
    let sun_dir = normalize(material.sun_direction);
    let sun_facing = saturate(dot(view_dir, sun_dir));
    let day = material.effective_daylight;
    let twilight = material.twilight_factor;
    let night = material.layer_night_factor;

    var base = mix(vec3<f32>(0.12, 0.14, 0.20), vec3<f32>(0.90, 0.92, 0.96), day);
    base = mix(base, base * vec3<f32>(0.62, 0.66, 0.74), (1.0 - sun_facing) * 0.45 * day);

    let warm = twilight * sun_facing * (0.35 + 0.65 * (1.0 - day));
    base = mix(base, material.sun_color.rgb * 1.05, warm * 0.75);
    base = mix(base, vec3<f32>(0.05, 0.07, 0.12), night);

    return base;
}

fn extinction_coefficient(density: f32) -> f32 {
    let night = material.layer_night_factor;
    let night_opacity = mix(0.35, 1.0, 1.0 - night * 0.65);
    return density * material.density_scale * night_opacity * EXTINCTION_ARTIFACT;
}

fn march_step_count(segment: f32) -> i32 {
    var steps = CLOUD_MARCH_MAX_STEPS;
    let baseline = segment / f32(steps);
    if baseline > CLOUD_MARCH_MAX_STEP {
        let needed = i32(ceil(segment / CLOUD_MARCH_MAX_STEP));
        steps = min(max(needed, CLOUD_MARCH_MAX_STEPS), CLOUD_MARCH_MAX_STEPS_CAP);
    }
    return steps;
}

fn march_step_len(segment: f32, steps: i32) -> f32 {
    let ideal = segment / f32(steps);
    // Short band slices must not clamp up to MIN_STEP; that pushes stratified samples past t_limit.
    if ideal < CLOUD_MARCH_MIN_STEP {
        return ideal;
    }
    return min(ideal, CLOUD_MARCH_MAX_STEP);
}

// Stratified spatial jitter per march step (frame=0): breaks equidistant sample planes.
// Offsets are screen-stable and step-index decorrelated; they do not alter world-space density.
fn march_sample_jitter(frag_coord: vec2<f32>, step_index: i32) -> f32 {
    let step_offset = vec2(f32(step_index) * 5.7, f32(step_index) * 11.3);
    return interleaved_gradient_noise(frag_coord + step_offset, 0u);
}

fn intersect_low_band(origin: vec3<f32>, direction: vec3<f32>) -> vec4<f32> {
    if abs(direction.y) < BAND_RAY_EPSILON {
        return vec4<f32>(-1.0);
    }

    let t0 = (material.low_y_min - origin.y) / direction.y;
    let t1 = (material.low_y_max - origin.y) / direction.y;
    let t_enter = min(t0, t1);
    let t_exit = max(t0, t1);
    if t_exit <= 0.0 || t_enter >= t_exit {
        return vec4<f32>(-1.0);
    }
    return vec4<f32>(max(t_enter, 0.0), t_exit, 0.0, 0.0);
}

fn render_view_origin() -> vec3<f32> {
    return view_bindings::view.world_position;
}

// Ray origin and inverse projection must share the same Bevy render View.
// Do not substitute a separately synchronized camera-position uniform.
fn fragment_world_ray(frag_coord: vec4<f32>) -> vec3<f32> {
    let origin = render_view_origin();
    let near_ndc = vec3<f32>(frag_coord_to_ndc(frag_coord).xy, 1.0);
    return normalize(position_ndc_to_world(near_ndc) - origin);
}

fn scene_ray_limit(origin: vec3<f32>, direction: vec3<f32>, frag_coord: vec4<f32>) -> f32 {
#ifdef DEPTH_PREPASS
    let scene_ndc = prepass_utils::prepass_depth(frag_coord, 0u);
    if scene_ndc <= SCENE_DEPTH_SKY_EPSILON {
        return SCENE_DEPTH_UNBOUNDED;
    }
    let scene_ndc_full = frag_coord_to_ndc(vec4<f32>(frag_coord.xy, scene_ndc, 1.0));
    let scene_world = position_ndc_to_world(scene_ndc_full);
    return max(dot(scene_world - origin, direction), 0.0);
#else
    return SCENE_DEPTH_UNBOUNDED;
#endif
}

fn march_low_clouds(direction: vec3<f32>, frag_coord: vec4<f32>) -> vec4<f32> {
    let origin = render_view_origin();
    let band = intersect_low_band(origin, direction);
    if band.x < 0.0 {
        return vec4<f32>(0.0);
    }

    let t_start = band.x;
    let t_exit = band.y;
    let t_scene = scene_ray_limit(origin, direction, frag_coord);
    if t_scene <= t_start {
        return vec4<f32>(0.0);
    }

    // Integrate only within the band, scene occlusion, and a segment cap measured from band entry.
    // Do not cap absolute camera distance: shallow horizon rays enter the band far away.
    let t_limit = min(min(t_exit, t_scene), t_start + CLOUD_MARCH_MAX_SEGMENT);
    if t_limit <= t_start {
        return vec4<f32>(0.0);
    }

    let segment = t_limit - t_start;
    let march_steps = march_step_count(segment);
    let step_len = march_step_len(segment, march_steps);

    var transmittance = 1.0;
    var accumulated = vec3<f32>(0.0);
    let sample_color = cloud_sample_color(direction);

    for (var i = 0; i < march_steps; i = i + 1) {
        let step_jitter = march_sample_jitter(frag_coord.xy, i);
        let t = t_start + (f32(i) + step_jitter) * step_len;
        if t > t_limit {
            break;
        }

        let sample_pos = origin + direction * t;
        let body = low_cloud_body_shape(sample_pos);
        if body.x <= 0.001 {
            // Cheap empty-space skip: L1+L2 probe only; next stratified sample handles spacing.
            continue;
        }

        let erosion = erosion_field(sample_pos, body.y);
        let density = saturate(remap(body.x, erosion * material.edge_breakup, 1.0, 0.0, 1.0));
        if density > 0.001 {
            // Cloud opacity is integrated from extinction over ray-segment length;
            // sample count is a quality setting, not the definition of cloud density.
            let sigma = extinction_coefficient(density);
            let segment_transmittance = exp(-sigma * step_len);
            let sample_weight = 1.0 - segment_transmittance;
            accumulated += transmittance * sample_weight * sample_color;
            transmittance *= segment_transmittance;
        }

        if transmittance < TRANSMITTANCE_CUTOFF {
            break;
        }
    }

    let cloud_alpha = 1.0 - transmittance;
    if cloud_alpha <= 0.001 {
        return vec4<f32>(0.0);
    }

    // Straight-alpha blend convention (`AlphaMode::Blend`): rgb is unpremultiplied cloud color.
    return vec4<f32>(accumulated / cloud_alpha, cloud_alpha);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0),
    );
    let clip_position = position_world_to_clip(out.world_position.xyz);
    // Proxy geometry provides fragment coverage only; it does not define the cloud ray.
    // Raster coverage only: reverse-Z far (NDC z = 0). Terrain ordering uses prepass in fragment.
    out.position = vec4<f32>(clip_position.xy, 0.0, clip_position.w);
    out.world_normal = normalize((world_from_local * vec4<f32>(vertex.position, 0.0)).xyz);

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    let direction = fragment_world_ray(in.position);

    out.color = march_low_clouds(direction, in.position);
    return out;
}
