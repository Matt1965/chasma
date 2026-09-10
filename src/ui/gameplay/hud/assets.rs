//! Chasma bottom HUD chrome: continuous backing plus per-plate frame art.
//!
//! The mockup's HUD is four octagonal plates at different heights, each with its
//! own chamfered corners and bronze trim. Earlier passes drew one rectangular
//! frame across the whole band and layered narrow seams over it, which could
//! only ever read as a rectangle with decorations on top.
//!
//! Ownership is now split:
//!
//! - Plate frames (see [`super::frames`]) own the silhouette: each is a
//!   nine-sliced octagon with real rails and interior, transparent outside
//!   the chamfer. Overlaps at joins keep interiors continuous.
//! - Endcap ornaments terminate the run at each end.
//!
//! There is no full-width rectangular backing. A rectangle behind the plates
//! is what made chamfers read as decorations on a bar.
//!
//! Nothing else draws a rail, so there is exactly one visible contour in any
//! region.

use bevy::prelude::*;
use bevy::sprite::{BorderRect, SliceScaleMode, TextureSlicer};
use bevy::ui::FocusPolicy;
use bevy::ui::widget::{ImageNode, NodeImageMode};

use super::super::styles::{HUD_ENDCAP_LEFT_WIDTH_PX, HUD_PLATE_CORNER_PX};
use super::geometry::{HudEndcapLeft, HudEndcapRight, HudEndcapSpire, HudViewportGeometry};

pub const HUD_PLATE_FRAME_PATH: &str = "images/ui/hud/plate_frame.png";
pub const HUD_PLATE_BACKING_PATH: &str = "images/ui/hud/plate_backing.png";
pub const HUD_ENDCAP_LEFT_PATH: &str = "images/ui/hud/endcap_left.png";
pub const HUD_ENDCAP_RIGHT_PATH: &str = "images/ui/hud/endcap_right.png";
pub const HUD_ENDCAP_SPIRE_PATH: &str = "images/ui/hud/endcap_spire.png";

/// Marker on per-plate interior chrome (the nine-sliced octagon itself).
#[derive(Component, Debug)]
pub struct HudFrameBackground;

/// Loaded HUD chrome handles (startup).
#[derive(Resource, Clone)]
pub struct HudUiAssets {
    pub plate_frame: Handle<Image>,
    pub plate_backing: Handle<Image>,
    pub endcap_left: Handle<Image>,
    pub endcap_right: Handle<Image>,
    pub endcap_spire: Handle<Image>,
}

impl HudUiAssets {
    pub fn load(asset_server: &AssetServer) -> Self {
        Self {
            plate_frame: asset_server.load(HUD_PLATE_FRAME_PATH),
            plate_backing: asset_server.load(HUD_PLATE_BACKING_PATH),
            endcap_left: asset_server.load(HUD_ENDCAP_LEFT_PATH),
            endcap_right: asset_server.load(HUD_ENDCAP_RIGHT_PATH),
            endcap_spire: asset_server.load(HUD_ENDCAP_SPIRE_PATH),
        }
    }

    /// Nine-slice for one octagonal plate. Corner blocks hold the chamfer and
    /// keep their source pixel size, so the cut corners stay crisp at any
    /// section width while the rails and interior stretch.
    pub fn plate_slicer() -> TextureSlicer {
        TextureSlicer {
            border: BorderRect {
                min_inset: Vec2::splat(HUD_PLATE_CORNER_PX),
                max_inset: Vec2::splat(HUD_PLATE_CORNER_PX),
            },
            center_scale_mode: SliceScaleMode::Stretch,
            sides_scale_mode: SliceScaleMode::Stretch,
            max_corner_scale: 1.0,
        }
    }
}

/// Spawn the ornamental noses that terminate the plate run at each end.
///
/// Endcaps are bottom-aligned and band height, so their contours line up with
/// the outer plates. The spire is the only art above the band; it sits over the
/// utility block, inboard of the right endcap.
pub fn spawn_hud_ornaments(parent: &mut ChildSpawnerCommands<'_>, assets: &HudUiAssets) {
    let geom = HudViewportGeometry::default();
    parent.spawn((
        HudEndcapLeft,
        ImageNode::new(assets.endcap_left.clone()),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Px(geom.endcap_left_width),
            height: Val::Percent(100.0),
            ..default()
        },
        FocusPolicy::Pass,
        ZIndex(-1),
    ));
    parent.spawn((
        HudEndcapRight,
        ImageNode::new(assets.endcap_right.clone()),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Px(geom.endcap_right_width),
            height: Val::Percent(100.0),
            ..default()
        },
        FocusPolicy::Pass,
        ZIndex(-1),
    ));
    parent.spawn((
        HudEndcapSpire,
        ImageNode::new(assets.endcap_spire.clone()),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(geom.endcap_right_width),
            bottom: Val::Percent(100.0),
            width: Val::Px(21.0 * geom.art_scale),
            height: Val::Px(96.0 * geom.art_scale),
            ..default()
        },
        FocusPolicy::Pass,
        ZIndex(-1),
    ));
}
