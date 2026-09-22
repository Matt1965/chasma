//! Shared presentation assets for derived building foundations.
//!
//! Material isolation stages (advance manually for regression bisection):
//! 1. `Solid` — obvious untextured color
//! 2. `Textured` — add stone texture only
//! 3. `TexturedTiledUv` — tiled generated UVs

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::buildings::assets::BUILDING_ASSET_ROOT;
use crate::buildings::foundation::FoundationUvMode;

/// Active foundation material isolation stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundationMaterialStage {
    Solid,
    Textured,
    TexturedTiledUv,
}

/// Active presentation stage. Textured + load confirmed; tiled UVs for visible stone pattern.
pub const FOUNDATION_MATERIAL_STAGE: FoundationMaterialStage =
    FoundationMaterialStage::TexturedTiledUv;

const FOUNDATION_STONE_TEXTURE_FILE: &str = "foundation_stone_wall.png";
const FOUNDATION_CONTROL_TEXTURE_FILE: &str = "foundation_control_white.png";

/// Active texture for textured stages (stone vs known-good control).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundationTextureKind {
    Stone,
    ControlWhite,
}

/// Texture under test for Stage B bisection. Switch to `ControlWhite` for control A/B/C.
pub const FOUNDATION_TEXTURE_KIND: FoundationTextureKind = FoundationTextureKind::Stone;

/// Obvious solid debug color for stage-A visibility bisection only.
pub const FOUNDATION_SOLID_DEBUG_COLOR: Color = Color::srgb(1.0, 0.45, 0.05);

/// Neutral multiply for textured stone (stage B/C).
pub const FOUNDATION_STONE_BASE_COLOR: Color = Color::srgb(0.92, 0.88, 0.82);

/// Cached foundation skirt material and stage metadata.
#[derive(Resource)]
pub struct FoundationPresentationAssets {
    pub material: Handle<StandardMaterial>,
    pub stage: FoundationMaterialStage,
    pub texture_kind: FoundationTextureKind,
    pub texture_path: String,
    pub texture: Option<Handle<Image>>,
}

impl FoundationPresentationAssets {
    pub fn debug_stub(material: Handle<StandardMaterial>) -> Self {
        Self {
            material,
            stage: FoundationMaterialStage::Solid,
            texture_kind: FoundationTextureKind::Stone,
            texture_path: String::new(),
            texture: None,
        }
    }
}

pub fn foundation_uv_mode_for_stage(stage: FoundationMaterialStage) -> FoundationUvMode {
    match stage {
        FoundationMaterialStage::Solid | FoundationMaterialStage::Textured => {
            FoundationUvMode::Trivial
        }
        FoundationMaterialStage::TexturedTiledUv => FoundationUvMode::Tiled,
    }
}

pub fn foundation_texture_asset_path_for(kind: FoundationTextureKind) -> String {
    let file = match kind {
        FoundationTextureKind::Stone => FOUNDATION_STONE_TEXTURE_FILE,
        FoundationTextureKind::ControlWhite => FOUNDATION_CONTROL_TEXTURE_FILE,
    };
    format!("{BUILDING_ASSET_ROOT}/{file}")
}

pub fn foundation_texture_asset_path() -> String {
    foundation_texture_asset_path_for(FOUNDATION_TEXTURE_KIND)
}

pub fn init_foundation_presentation_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let stage = FOUNDATION_MATERIAL_STAGE;
    let texture_kind = FOUNDATION_TEXTURE_KIND;
    let texture_path = foundation_texture_asset_path_for(texture_kind);
    let texture = if stage == FoundationMaterialStage::Solid {
        None
    } else {
        Some(asset_server.load(texture_path.clone()))
    };
    // Bind the texture only after AssetServer reports Loaded (see
    // `sync_foundation_texture_binding`). Until then the skirt stays visible via base_color.
    let material = foundation_material_for_stage(stage, None, &mut materials);

    commands.insert_resource(FoundationPresentationAssets {
        material,
        stage,
        texture_kind,
        texture_path,
        texture,
    });
}

pub fn foundation_material_for_stage(
    stage: FoundationMaterialStage,
    texture: Option<Handle<Image>>,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    match stage {
        FoundationMaterialStage::Solid => materials.add(StandardMaterial {
            base_color: FOUNDATION_SOLID_DEBUG_COLOR,
            base_color_texture: None,
            alpha_mode: AlphaMode::Opaque,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        FoundationMaterialStage::Textured | FoundationMaterialStage::TexturedTiledUv => {
            materials.add(StandardMaterial {
                base_color: FOUNDATION_STONE_BASE_COLOR,
                base_color_texture: texture,
                alpha_mode: AlphaMode::Opaque,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        }
    }
}

/// Attach `base_color_texture` once the image is resident; keep tint-only while loading.
pub fn sync_foundation_texture_binding(
    asset_server: Res<AssetServer>,
    foundation_assets: Res<FoundationPresentationAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut load_reported: Local<Option<String>>,
) {
    if foundation_assets.stage == FoundationMaterialStage::Solid {
        return;
    }
    let Some(texture) = foundation_assets.texture.clone() else {
        return;
    };
    let Some(material) = materials.get_mut(&foundation_assets.material) else {
        return;
    };

    let load_state = asset_server.get_load_state(&texture);
    let state_label = match load_state {
        Some(LoadState::Loaded) => "loaded",
        Some(LoadState::Loading) => "loading",
        Some(LoadState::NotLoaded) => "not_loaded",
        Some(LoadState::Failed(_)) => "failed",
        None => "unknown",
    };
    if load_reported.as_deref() != Some(state_label) {
        match load_state {
            Some(LoadState::Failed(_)) => {
                warn!(
                    "FOUNDATION texture `{}` load state: {}",
                    foundation_assets.texture_path,
                    state_label
                );
            }
            Some(LoadState::Loaded) => {
                info!(
                    "FOUNDATION texture `{}` load state: {}",
                    foundation_assets.texture_path,
                    state_label
                );
            }
            _ => {
                debug!(
                    "FOUNDATION texture `{}` load state: {}",
                    foundation_assets.texture_path,
                    state_label
                );
            }
        }
        *load_reported = Some(state_label.to_string());
    }

    match load_state {
        Some(LoadState::Loaded) => {
            if material.base_color_texture.as_ref() != Some(&texture) {
                material.base_color_texture = Some(texture);
            }
        }
        Some(LoadState::Failed(_)) => {
            material.base_color_texture = None;
        }
        _ => {
            material.base_color_texture = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_stage_material_has_no_texture() {
        let mut materials = Assets::<StandardMaterial>::default();
        let handle = foundation_material_for_stage(FoundationMaterialStage::Solid, None, &mut materials);
        let material = materials.get(&handle).expect("foundation material");
        assert!(material.base_color_texture.is_none());
        assert_eq!(material.base_color, FOUNDATION_SOLID_DEBUG_COLOR);
        assert!(material.unlit);
        assert!(material.double_sided);
        assert_eq!(material.alpha_mode, AlphaMode::Opaque);
    }

    #[test]
    fn textured_stage_material_references_stone_texture() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<Assets<StandardMaterial>>();
        app.init_asset::<Image>();
        let texture = {
            let asset_server = app.world().resource::<AssetServer>();
            asset_server.load(foundation_texture_asset_path())
        };
        let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
        let handle = foundation_material_for_stage(
            FoundationMaterialStage::Textured,
            Some(texture.clone()),
            &mut materials,
        );
        let material = materials.get(&handle).expect("foundation material");
        assert_eq!(material.base_color, FOUNDATION_STONE_BASE_COLOR);
        assert_eq!(material.base_color_texture, Some(texture));
        assert!(material.unlit);
        assert!(material.double_sided);
        assert_eq!(material.alpha_mode, AlphaMode::Opaque);
    }

    #[test]
    fn active_stage_is_tiled_after_texture_load_confirmed() {
        assert_eq!(
            FOUNDATION_MATERIAL_STAGE,
            FoundationMaterialStage::TexturedTiledUv
        );
    }

    #[test]
    fn stone_texture_asset_path_is_under_buildings_root() {
        let path = foundation_texture_asset_path_for(FoundationTextureKind::Stone);
        assert!(path.starts_with("buildings/"));
        assert!(path.ends_with("foundation_stone_wall.png"));
    }

    #[test]
    fn control_texture_asset_path_is_under_buildings_root() {
        let path = foundation_texture_asset_path_for(FoundationTextureKind::ControlWhite);
        assert!(path.starts_with("buildings/"));
        assert!(path.ends_with("foundation_control_white.png"));
    }

    #[test]
    fn stone_texture_file_exists_on_disk() {
        let path = std::path::Path::new("assets")
            .join(foundation_texture_asset_path_for(FoundationTextureKind::Stone));
        assert!(path.is_file(), "missing stone texture at {}", path.display());
    }

}
