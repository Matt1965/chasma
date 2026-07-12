use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::world::{
    AnimationClipKey, AnimationProfileCatalog, UnitCatalog, UnitDefinitionId, WeaponCatalog,
    WeaponDefinitionId,
};

use super::components::{
    AnimationPlaybackPending, AnimationProfileHandle, UnitAnimationGraphInstalled,
    UnitAnimationPlayerLink,
};
use super::layers::{FULL_BODY_CLIP_MASK, LOWER_BODY_CLIP_MASK, UPPER_BODY_CLIP_MASK};
use super::validation::{
    AnimationValidationIndex, DefinitionValidationReport, validate_definition_animation_assets,
};
use crate::units::assets::gltf_asset_path;
use crate::units::components::{UnitRenderEntity, UnitRenderMetadata};

/// Identity for shared animation graph assets (A6).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimationGraphShareKey {
    pub profile_id: crate::world::AnimationProfileId,
    pub gltf_asset_path: String,
    pub default_weapon_id: WeaponDefinitionId,
}

/// Built animation graph assets for one unit definition (A1/A2).
#[derive(Debug, Clone)]
pub struct DefinitionAnimationGraph {
    pub graph: Handle<AnimationGraph>,
    pub locomotion_nodes: HashMap<AnimationClipKey, AnimationNodeIndex>,
    pub attack_nodes: HashMap<WeaponDefinitionId, AnimationNodeIndex>,
    pub locomotion_durations: HashMap<AnimationClipKey, f32>,
    pub attack_durations: HashMap<WeaponDefinitionId, f32>,
    pub death_node: Option<AnimationNodeIndex>,
    pub death_duration: Option<f32>,
    pub hit_reaction_node: Option<AnimationNodeIndex>,
    pub hit_reaction_duration: Option<f32>,
    /// Idle fallback node for missing attack clips (A2).
    pub idle_fallback_node: Option<AnimationNodeIndex>,
    /// Additive blend root for masked layering (A4).
    pub blend_root: AnimationNodeIndex,
    pub profile_id: crate::world::AnimationProfileId,
    pub share_key: AnimationGraphShareKey,
}

/// Runtime animation asset cache: glTF handles and shared graphs (A1/A6).
#[derive(Resource, Debug)]
pub struct UnitAnimationAssets {
    gltfs: HashMap<UnitDefinitionId, Handle<Gltf>>,
    gltf_by_path: HashMap<String, Handle<Gltf>>,
    graphs: HashMap<UnitDefinitionId, DefinitionAnimationGraph>,
    shared_graphs: HashMap<AnimationGraphShareKey, DefinitionAnimationGraph>,
    share_keys: HashMap<UnitDefinitionId, AnimationGraphShareKey>,
    warned: HashSet<String>,
    pub validation: AnimationValidationIndex,
}

impl Default for UnitAnimationAssets {
    fn default() -> Self {
        Self {
            gltfs: HashMap::new(),
            gltf_by_path: HashMap::new(),
            graphs: HashMap::new(),
            shared_graphs: HashMap::new(),
            share_keys: HashMap::new(),
            warned: HashSet::new(),
            validation: AnimationValidationIndex::default(),
        }
    }
}

impl UnitAnimationAssets {
    pub fn gltf_for(&self, definition_id: &UnitDefinitionId) -> Option<&Handle<Gltf>> {
        self.gltfs.get(definition_id)
    }

    pub fn graph_for(&self, definition_id: &UnitDefinitionId) -> Option<&DefinitionAnimationGraph> {
        self.graphs.get(definition_id)
    }

    pub fn share_key_for(
        &self,
        definition_id: &UnitDefinitionId,
    ) -> Option<&AnimationGraphShareKey> {
        self.share_keys.get(definition_id)
    }

    pub fn shared_graph_count(&self) -> usize {
        self.shared_graphs.len()
    }

    pub fn definition_graph_count(&self) -> usize {
        self.graphs.len()
    }

    pub fn validation_for(
        &self,
        definition_id: &UnitDefinitionId,
    ) -> Option<&DefinitionValidationReport> {
        self.validation.report_for(definition_id)
    }

    pub fn log_once(&mut self, key: impl Into<String>) {
        let key = key.into();
        if self.warned.insert(key.clone()) {
            warn!("unit animation: {key}");
        }
    }

    #[cfg(test)]
    pub fn insert_test_graph(
        &mut self,
        definition_id: UnitDefinitionId,
        graph: DefinitionAnimationGraph,
    ) {
        self.share_keys
            .insert(definition_id.clone(), graph.share_key.clone());
        self.shared_graphs
            .insert(graph.share_key.clone(), graph.clone());
        self.graphs.insert(definition_id, graph);
    }
}

pub(crate) fn gltf_asset_path_for_definition(
    definition: &crate::world::UnitDefinition,
) -> Option<String> {
    gltf_asset_path(&definition.render_key).map(|path| path.to_string())
}

/// Preload glTF asset handles for animated unit definitions (A1/A6).
pub fn preload_unit_animation_gltfs(
    catalog: &UnitCatalog,
    profiles: &AnimationProfileCatalog,
    asset_server: &AssetServer,
) -> UnitAnimationAssets {
    let mut gltfs = HashMap::new();
    let mut gltf_by_path = HashMap::new();
    for definition in catalog.definitions() {
        if definition.animation_profile_id.is_none() {
            continue;
        }
        let Some(profile_id) = &definition.animation_profile_id else {
            continue;
        };
        if profiles.get(profile_id).is_none() {
            continue;
        }
        let Some(path) = gltf_asset_path_for_definition(definition) else {
            continue;
        };
        let handle = gltf_by_path
            .entry(path.clone())
            .or_insert_with(|| asset_server.load(path.clone()))
            .clone();
        gltfs.insert(definition.id.clone(), handle);
    }
    UnitAnimationAssets {
        gltfs,
        gltf_by_path,
        ..Default::default()
    }
}

struct GraphBuildContext<'a> {
    profile: &'a crate::world::AnimationProfile,
    profile_id: &'a crate::world::AnimationProfileId,
    definition: &'a crate::world::UnitDefinition,
    gltf: &'a Gltf,
    weapon: Option<&'a crate::world::WeaponDefinition>,
    clips: &'a Assets<AnimationClip>,
    assets: &'a mut UnitAnimationAssets,
}

/// Typed clip resolution before graph node assembly (A1 regression fix).
#[derive(Debug, Default)]
struct ResolvedClipSet {
    locomotion: Vec<(AnimationClipKey, Handle<AnimationClip>, f32)>,
    attacks: Vec<(WeaponDefinitionId, Handle<AnimationClip>, f32)>,
    death: Option<(Handle<AnimationClip>, f32)>,
    hit: Option<(Handle<AnimationClip>, f32)>,
}

fn resolve_clips_for_graph(ctx: &mut GraphBuildContext<'_>) -> ResolvedClipSet {
    let mut resolved = ResolvedClipSet::default();

    for key in [
        AnimationClipKey::Idle,
        AnimationClipKey::Walk,
        AnimationClipKey::Run,
        AnimationClipKey::TurnLeft,
        AnimationClipKey::TurnRight,
    ] {
        let Some((clip_name, _resolved)) = ctx.profile.resolve_clip_name(key) else {
            continue;
        };
        let Some(handle) = ctx.gltf.named_animations.get(clip_name) else {
            ctx.assets.log_once(format!(
                "unit `{}` profile `{}` missing clip `{clip_name}` in glTF",
                ctx.definition.id.as_str(),
                ctx.profile_id.as_str()
            ));
            continue;
        };
        let duration = ctx
            .clips
            .get(handle)
            .map(|clip| clip.duration())
            .unwrap_or(1.0);
        resolved.locomotion.push((key, handle.clone(), duration));
    }

    if let Some(weapon) = ctx.weapon {
        let clip_name = weapon.animation_key.trim();
        if !clip_name.is_empty() {
            if let Some(handle) = ctx.gltf.named_animations.get(clip_name) {
                let duration = ctx
                    .clips
                    .get(handle)
                    .map(|clip| clip.duration())
                    .unwrap_or(1.0);
                resolved
                    .attacks
                    .push((weapon.id.clone(), handle.clone(), duration));
            } else {
                ctx.assets.log_once(format!(
                    "unit `{}` weapon `{}` missing attack clip `{clip_name}` in glTF",
                    ctx.definition.id.as_str(),
                    weapon.id.as_str()
                ));
            }
        } else {
            ctx.assets.log_once(format!(
                "unit `{}` weapon `{}` has blank attack animation",
                ctx.definition.id.as_str(),
                weapon.id.as_str()
            ));
        }
    }

    if let Some(clip_name) = ctx.profile.resolve_death_clip_name() {
        if let Some(handle) = ctx.gltf.named_animations.get(clip_name) {
            let duration = ctx
                .clips
                .get(handle)
                .map(|clip| clip.duration())
                .unwrap_or(1.0);
            resolved.death = Some((handle.clone(), duration));
        } else {
            ctx.assets.log_once(format!(
                "unit `{}` profile `{}` missing death clip `{clip_name}` in glTF",
                ctx.definition.id.as_str(),
                ctx.profile_id.as_str()
            ));
        }
    }

    if let Some(clip_name) = ctx.profile.resolve_hit_reaction_clip_name() {
        if let Some(handle) = ctx.gltf.named_animations.get(clip_name) {
            let duration = ctx
                .clips
                .get(handle)
                .map(|clip| clip.duration())
                .unwrap_or(1.0);
            resolved.hit = Some((handle.clone(), duration));
        } else {
            ctx.assets.log_once(format!(
                "unit `{}` profile `{}` missing hit clip `{clip_name}` in glTF",
                ctx.definition.id.as_str(),
                ctx.profile_id.as_str()
            ));
        }
    }

    resolved
}

fn assemble_graph_from_clips(
    resolved: &ResolvedClipSet,
    profile_id: &crate::world::AnimationProfileId,
    share_key: AnimationGraphShareKey,
    graphs_assets: &mut Assets<AnimationGraph>,
) -> Option<DefinitionAnimationGraph> {
    if resolved.locomotion.is_empty()
        && resolved.attacks.is_empty()
        && resolved.death.is_none()
        && resolved.hit.is_none()
    {
        return None;
    }

    let mut graph = AnimationGraph::new();
    let blend_root = graph.add_additive_blend(1.0, graph.root);

    let mut locomotion_nodes = HashMap::new();
    let mut locomotion_durations = HashMap::new();
    for (key, handle, duration) in &resolved.locomotion {
        let node = graph.add_clip_with_mask(handle.clone(), LOWER_BODY_CLIP_MASK, 1.0, blend_root);
        locomotion_nodes.insert(*key, node);
        locomotion_durations.insert(*key, *duration);
    }

    let mut attack_nodes = HashMap::new();
    let mut attack_durations = HashMap::new();
    for (weapon_id, handle, duration) in &resolved.attacks {
        let node = graph.add_clip_with_mask(handle.clone(), UPPER_BODY_CLIP_MASK, 1.0, blend_root);
        attack_nodes.insert(weapon_id.clone(), node);
        attack_durations.insert(weapon_id.clone(), *duration);
    }

    let death_node = resolved.death.as_ref().map(|(handle, _)| {
        graph.add_clip_with_mask(handle.clone(), FULL_BODY_CLIP_MASK, 1.0, blend_root)
    });
    let death_duration = resolved.death.as_ref().map(|(_, duration)| *duration);

    let hit_reaction_node = resolved.hit.as_ref().map(|(handle, _)| {
        graph.add_clip_with_mask(handle.clone(), FULL_BODY_CLIP_MASK, 1.0, blend_root)
    });
    let hit_reaction_duration = resolved.hit.as_ref().map(|(_, duration)| *duration);

    let idle_fallback_node = locomotion_nodes.get(&AnimationClipKey::Idle).copied();
    let graph_handle = graphs_assets.add(graph);

    Some(DefinitionAnimationGraph {
        graph: graph_handle,
        locomotion_nodes,
        attack_nodes,
        locomotion_durations,
        attack_durations,
        death_node,
        death_duration,
        hit_reaction_node,
        hit_reaction_duration,
        idle_fallback_node,
        blend_root,
        profile_id: profile_id.clone(),
        share_key,
    })
}

fn build_graph_from_context(
    ctx: &mut GraphBuildContext<'_>,
    graphs_assets: &mut Assets<AnimationGraph>,
) -> Option<DefinitionAnimationGraph> {
    let path = gltf_asset_path_for_definition(ctx.definition)?;
    let share_key = AnimationGraphShareKey {
        profile_id: ctx.profile_id.clone(),
        gltf_asset_path: path,
        default_weapon_id: ctx.definition.default_weapon_id.clone(),
    };

    if let Some(shared) = ctx.assets.shared_graphs.get(&share_key) {
        return Some(shared.clone());
    }

    let resolved = resolve_clips_for_graph(ctx);
    if resolved.locomotion.is_empty()
        && resolved.attacks.is_empty()
        && resolved.death.is_none()
        && resolved.hit.is_none()
    {
        ctx.assets.log_once(format!(
            "unit `{}` has no resolvable animation clips",
            ctx.definition.id.as_str()
        ));
        return None;
    }

    let Some(built) =
        assemble_graph_from_clips(&resolved, ctx.profile_id, share_key.clone(), graphs_assets)
    else {
        return None;
    };
    ctx.assets.shared_graphs.insert(share_key, built.clone());
    Some(built)
}

/// Build shared [`AnimationGraph`] assets once glTF data is available (A1/A2/A6).
pub fn build_unit_animation_graphs(
    catalog: Res<UnitCatalog>,
    profiles: Res<AnimationProfileCatalog>,
    weapons: Res<WeaponCatalog>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs_assets: ResMut<Assets<AnimationGraph>>,
    mut assets: ResMut<UnitAnimationAssets>,
) {
    for definition in catalog.definitions() {
        if assets.graphs.contains_key(&definition.id) {
            continue;
        }
        let Some(profile_id) = &definition.animation_profile_id else {
            continue;
        };
        let profile = profiles.get(profile_id);
        let gltf_handle = assets.gltfs.get(&definition.id).cloned();
        let gltf = gltf_handle.as_ref().and_then(|handle| gltfs.get(handle));
        let weapon = weapons.get(&definition.default_weapon_id);
        let weapon_clip = weapon.map(|value| value.animation_key.as_str());
        let report = validate_definition_animation_assets(definition, profile, gltf, weapon_clip);
        assets.validation.log_new_issues(&report);
        assets
            .validation
            .reports
            .insert(definition.id.clone(), report);

        let Some(profile) = profile else {
            assets.log_once(format!(
                "missing animation profile `{}` for unit `{}`",
                profile_id.as_str(),
                definition.id.as_str()
            ));
            continue;
        };
        let Some(gltf_handle) = gltf_handle else {
            continue;
        };
        if !asset_server.is_loaded_with_dependencies(&gltf_handle) {
            continue;
        }
        let Some(gltf) = gltf else {
            continue;
        };

        let mut ctx = GraphBuildContext {
            profile,
            profile_id,
            definition,
            gltf,
            weapon,
            clips: &clips,
            assets: &mut assets,
        };
        let Some(built) = build_graph_from_context(&mut ctx, &mut graphs_assets) else {
            continue;
        };
        assets
            .share_keys
            .insert(definition.id.clone(), built.share_key.clone());
        assets.graphs.insert(definition.id.clone(), built);
    }
}

/// Resolve definition id from live world data or render metadata (A1 corpse install).
pub(crate) fn resolve_presentation_definition_id(
    marker: Option<&UnitRenderEntity>,
    metadata: Option<&UnitRenderMetadata>,
    world: &crate::world::WorldData,
) -> Option<UnitDefinitionId> {
    if let Some(marker) = marker {
        if let Some(record) = world.get_unit(marker.unit_id) {
            return Some(record.definition_id.clone());
        }
    }
    metadata.map(|value| value.definition_id.clone())
}

/// Install graph + transitions on a discovered player entity (A1).
pub fn install_animation_graph_on_player(
    mut commands: Commands,
    roots: Query<
        (
            Entity,
            &UnitAnimationPlayerLink,
            Option<&UnitRenderEntity>,
            Option<&UnitRenderMetadata>,
        ),
        Without<UnitAnimationGraphInstalled>,
    >,
    players: Query<Entity, With<AnimationPlayer>>,
    world: Res<crate::world::WorldData>,
    catalog: Res<UnitCatalog>,
    assets: Res<UnitAnimationAssets>,
) {
    for (root, link, marker, metadata) in &roots {
        if players.get(link.player_entity).is_err() {
            continue;
        }
        let Some(definition_id) = resolve_presentation_definition_id(marker, metadata, &world)
        else {
            continue;
        };
        let Some(definition) = catalog.get(&definition_id) else {
            continue;
        };
        let Some(built) = assets.graph_for(&definition.id) else {
            continue;
        };

        commands.entity(link.player_entity).insert((
            AnimationGraphHandle(built.graph.clone()),
            AnimationTransitions::new(),
            UnitAnimationGraphInstalled,
            AnimationProfileHandle {
                profile_id: built.profile_id.clone(),
            },
        ));
        commands
            .entity(root)
            .insert((UnitAnimationGraphInstalled, AnimationPlaybackPending));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{AnimationProfile, AnimationProfileId, UnitRenderKey};

    fn share_key(profile: &str, path: &str, weapon: &str) -> AnimationGraphShareKey {
        AnimationGraphShareKey {
            profile_id: AnimationProfileId::new(profile),
            gltf_asset_path: path.to_string(),
            default_weapon_id: WeaponDefinitionId::new(weapon),
        }
    }

    fn empty_graph(share_key: AnimationGraphShareKey) -> DefinitionAnimationGraph {
        DefinitionAnimationGraph {
            graph: Handle::default(),
            locomotion_nodes: Default::default(),
            attack_nodes: Default::default(),
            locomotion_durations: Default::default(),
            attack_durations: Default::default(),
            death_node: None,
            death_duration: None,
            hit_reaction_node: None,
            hit_reaction_duration: None,
            idle_fallback_node: None,
            blend_root: AnimationNodeIndex::new(0),
            profile_id: share_key.profile_id.clone(),
            share_key,
        }
    }

    #[test]
    fn identical_share_keys_reuse_one_shared_graph() {
        let key = share_key("humanoid", "units/wolf.glb", "weapon_wolf_bite");
        let graph_a = empty_graph(key.clone());
        let mut assets = UnitAnimationAssets::default();
        assets.shared_graphs.insert(key.clone(), graph_a.clone());
        assets
            .graphs
            .insert(UnitDefinitionId::new("wolf_a"), graph_a.clone());
        assets
            .graphs
            .insert(UnitDefinitionId::new("wolf_b"), graph_a);
        assets
            .share_keys
            .insert(UnitDefinitionId::new("wolf_a"), key.clone());
        assets
            .share_keys
            .insert(UnitDefinitionId::new("wolf_b"), key);
        assert_eq!(assets.shared_graph_count(), 1);
        assert_eq!(assets.definition_graph_count(), 2);
    }

    #[test]
    fn different_weapons_produce_distinct_share_keys() {
        let key_a = share_key("humanoid", "units/wolf.glb", "weapon_a");
        let key_b = share_key("humanoid", "units/wolf.glb", "weapon_b");
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn gltf_path_dedupes_handles() {
        let mut assets = UnitAnimationAssets::default();
        let handle: Handle<Gltf> = Handle::default();
        assets
            .gltf_by_path
            .insert("units/wolf.glb".to_string(), handle.clone());
        assets
            .gltfs
            .insert(UnitDefinitionId::new("wolf_a"), handle.clone());
        assets.gltfs.insert(UnitDefinitionId::new("wolf_b"), handle);
        assert_eq!(assets.gltf_by_path.len(), 1);
    }

    fn clip_handle() -> Handle<AnimationClip> {
        Handle::default()
    }

    fn resolved_both_presentation_clips() -> ResolvedClipSet {
        ResolvedClipSet {
            locomotion: vec![(AnimationClipKey::Idle, clip_handle(), 1.0)],
            attacks: Vec::new(),
            death: Some((clip_handle(), 2.0)),
            hit: Some((clip_handle(), 0.5)),
        }
    }

    #[test]
    fn death_and_hit_graph_nodes_are_distinct() {
        let mut graphs = Assets::<AnimationGraph>::default();
        let share_key = share_key("humanoid", "units/wolf.glb", "weapon_wolf_bite");
        let built = assemble_graph_from_clips(
            &resolved_both_presentation_clips(),
            &AnimationProfileId::new("humanoid"),
            share_key,
            &mut graphs,
        )
        .expect("graph");
        assert_ne!(built.death_node, built.hit_reaction_node);
        assert!(built.death_node.is_some());
        assert!(built.hit_reaction_node.is_some());
    }

    #[test]
    fn death_only_graph_maps_death_node() {
        let mut graphs = Assets::<AnimationGraph>::default();
        let share_key = share_key("humanoid", "units/wolf.glb", "weapon_wolf_bite");
        let resolved = ResolvedClipSet {
            locomotion: vec![(AnimationClipKey::Idle, clip_handle(), 1.0)],
            attacks: Vec::new(),
            death: Some((clip_handle(), 2.0)),
            hit: None,
        };
        let built = assemble_graph_from_clips(
            &resolved,
            &AnimationProfileId::new("humanoid"),
            share_key,
            &mut graphs,
        )
        .unwrap();
        assert!(built.death_node.is_some());
        assert!(built.hit_reaction_node.is_none());
    }

    #[test]
    fn hit_only_graph_maps_hit_node() {
        let mut graphs = Assets::<AnimationGraph>::default();
        let share_key = share_key("humanoid", "units/wolf.glb", "weapon_wolf_bite");
        let resolved = ResolvedClipSet {
            locomotion: vec![(AnimationClipKey::Idle, clip_handle(), 1.0)],
            attacks: Vec::new(),
            death: None,
            hit: Some((clip_handle(), 0.5)),
        };
        let built = assemble_graph_from_clips(
            &resolved,
            &AnimationProfileId::new("humanoid"),
            share_key,
            &mut graphs,
        )
        .unwrap();
        assert!(built.death_node.is_none());
        assert!(built.hit_reaction_node.is_some());
    }

    #[test]
    fn missing_optional_presentation_clips_do_not_shift_attack_mapping() {
        let mut graphs = Assets::<AnimationGraph>::default();
        let share_key = share_key("humanoid", "units/wolf.glb", "weapon_wolf_bite");
        let resolved = ResolvedClipSet {
            locomotion: vec![(AnimationClipKey::Idle, clip_handle(), 1.0)],
            attacks: vec![(
                WeaponDefinitionId::new("weapon_wolf_bite"),
                clip_handle(),
                1.2,
            )],
            death: None,
            hit: None,
        };
        let built = assemble_graph_from_clips(
            &resolved,
            &AnimationProfileId::new("humanoid"),
            share_key,
            &mut graphs,
        )
        .unwrap();
        assert!(
            built
                .attack_nodes
                .contains_key(&WeaponDefinitionId::new("weapon_wolf_bite"))
        );
    }

    #[test]
    fn corpse_definition_resolves_from_metadata() {
        use crate::units::components::{UnitRenderEntity, UnitRenderMetadata};
        use crate::world::{ChunkLayout, UnitDefinitionId, UnitId, WorldData};

        let world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let marker = UnitRenderEntity {
            unit_id: UnitId::new(99),
        };
        let metadata = UnitRenderMetadata {
            definition_id: UnitDefinitionId::new("wolf"),
        };
        assert_eq!(
            resolve_presentation_definition_id(Some(&marker), Some(&metadata), &world),
            Some(UnitDefinitionId::new("wolf"))
        );
    }
}
