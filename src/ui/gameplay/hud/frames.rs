//! Per-section octagonal plate frames — the HUD's visible silhouette.
//!
//! The mockup's four plates sit at different heights, which is what makes the
//! run read as assembled hardware rather than one bar. Each plate owns its own
//! top/bottom contour, chamfered corners, and bronze trim.
//!
//! # Transitions
//!
//! Interior joins push the nine-slice past the clip by [`HUD_PLATE_JOIN_OVERLAP_PX`],
//! trimming the deep ornamental cut to the shallow [`HUD_PLATE_JOIN_CHAMFER_PX`].
//! Both plates extend past the boundary so the interior stays continuous.
//!
//! Outer plates tuck [`HUD_ENDCAP_SEAT_PX`] under the endcaps so the top and
//! bottom rails continue into the nose instead of leaving a corner gap.
//!
//! # Layout safety
//!
//! Plates are absolutely positioned children of the HUD root, so they never
//! consume flex width. Vertical insets are constants relative to the root, not
//! derived from content.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::ui::widget::{ImageNode, NodeImageMode};
use bevy::ui::{ComputedNode, Overflow, UiGlobalTransform};

use super::super::command_panel::CommandPanelRoot;
use super::super::selected_unit_panel::SelectedUnitPanelRoot;
use super::super::squad_panel::SquadPanelRoot;
use super::super::styles::{HUD_ENDCAP_SEAT_PX, HUD_PLATE_JOIN_OVERLAP_PX};
use super::super::utility_panel::UtilityPanelRoot;
use super::assets::{HudFrameBackground, HudUiAssets};
use super::geometry::HudViewportGeometry;

/// One HUD plate in the run, left to right.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudPlateSection {
    Selected,
    Roster,
    Commands,
    Utility,
}

impl HudPlateSection {
    pub const ALL: [Self; 4] = [Self::Selected, Self::Roster, Self::Commands, Self::Utility];

    /// Vertical insets from the HUD root, mirroring the mockup's stepped
    /// contour: tallest at the selected plate, shortest at commands, rising
    /// again under the utility spire.
    ///
    /// Capped so every plate still contains the shared content row
    /// (`HUD_FRAME_TOP_PX` / `HUD_FRAME_BOTTOM_PX`).
    pub fn insets_px(self) -> (f32, f32) {
        match self {
            Self::Selected => (0.0, 0.0),
            Self::Roster => (3.0, 8.0),
            Self::Commands => (11.0, 13.0),
            Self::Utility => (4.0, 1.0),
        }
    }

    pub fn height_px(self, hud_height: f32) -> f32 {
        let (top, bottom) = self.insets_px();
        hud_height - top - bottom
    }

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|section| *section == self)
            .expect("section is in ALL")
    }

    /// Which sides meet another plate.
    pub fn interior_edges(self) -> (bool, bool) {
        let index = self.index();
        (index > 0, index + 1 < Self::ALL.len())
    }

    /// Shorter plates draw last so a transition reads as one step down onto the
    /// taller neighbour's rail, instead of the taller plate's cut edge showing
    /// through on top.
    pub fn z_index(self) -> i32 {
        match self {
            Self::Selected => -6,
            Self::Utility => -5,
            Self::Roster => -4,
            Self::Commands => -3,
        }
    }
}

/// Clip container for one plate; positioned from resolved section geometry.
#[derive(Component, Debug)]
pub struct HudPlateFrame {
    pub section: HudPlateSection,
}

/// The nine-sliced frame image inside a [`HudPlateFrame`] container.
#[derive(Component, Debug)]
struct HudPlateFrameArt;

/// Spawn one inert plate frame per section on the HUD root.
pub fn spawn_hud_plate_frames(parent: &mut ChildSpawnerCommands<'_>, assets: &HudUiAssets) {
    let hud_height = HudViewportGeometry::default().hud_height;
    for section in HudPlateSection::ALL {
        let (top, bottom) = section.insets_px();
        let (interior_left, interior_right) = section.interior_edges();
        // Push the image past the clip on interior sides, trimming the deep
        // ornamental cut down to the shallow join chamfer.
        let overhang = |interior: bool| {
            if interior {
                HUD_PLATE_JOIN_OVERLAP_PX
            } else {
                0.0
            }
        };
        let (overhang_left, overhang_right) = (overhang(interior_left), overhang(interior_right));

        parent
            .spawn((
                HudPlateFrame { section },
                HudFrameBackground,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(top),
                    left: Val::Px(0.0),
                    width: Val::Px(0.0),
                    height: Val::Px(hud_height - top - bottom),
                    overflow: Overflow::clip(),
                    ..default()
                },
                FocusPolicy::Pass,
                ZIndex(section.z_index()),
            ))
            .with_children(|plate| {
                plate.spawn((
                    HudPlateFrameArt,
                    ImageNode {
                        image: assets.plate_frame.clone(),
                        image_mode: NodeImageMode::Sliced(HudUiAssets::plate_slicer()),
                        ..default()
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(-overhang_left),
                        right: Val::Px(-overhang_right),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        ..default()
                    },
                    FocusPolicy::Pass,
                ));
            });
    }
}

fn section_span<M: Component>(
    query: &Query<(&ComputedNode, &UiGlobalTransform), With<M>>,
) -> Option<(f32, f32)> {
    query.iter().next().map(|(computed, transform)| {
        let half = computed.size().x * 0.5;
        (
            transform.translation.x - half,
            transform.translation.x + half,
        )
    })
}

/// Stretch each plate frame across its section after UI layout resolves.
pub fn sync_hud_plate_frames(
    geometry: Res<HudViewportGeometry>,
    roots: Query<&ComputedNode, With<super::super::layout::GameplayHudRoot>>,
    selected: Query<(&ComputedNode, &UiGlobalTransform), With<SelectedUnitPanelRoot>>,
    roster: Query<(&ComputedNode, &UiGlobalTransform), With<SquadPanelRoot>>,
    commands: Query<(&ComputedNode, &UiGlobalTransform), With<CommandPanelRoot>>,
    utility: Query<(&ComputedNode, &UiGlobalTransform), With<UtilityPanelRoot>>,
    mut frames: Query<(&HudPlateFrame, &mut Node)>,
) {
    let Ok(root) = roots.single() else {
        return;
    };
    let root_width = root.size().x;
    if root_width <= 0.0 {
        return;
    }
    let endcap_left = geometry.endcap_left_width;
    let endcap_right = geometry.endcap_right_width;

    let spans = [
        (HudPlateSection::Selected, section_span(&selected)),
        (HudPlateSection::Roster, section_span(&roster)),
        (HudPlateSection::Commands, section_span(&commands)),
        (HudPlateSection::Utility, section_span(&utility)),
    ];

    for (section, span) in spans {
        let Some((min_x, max_x)) = span else {
            continue;
        };
        let (interior_left, interior_right) = section.interior_edges();

        // Interior edges overlap so join chamfers land on plate body. Outer
        // edges tuck under the endcap noses so the rails meet without a gap.
        let left = if interior_left {
            min_x - HUD_PLATE_JOIN_OVERLAP_PX
        } else {
            (endcap_left - HUD_ENDCAP_SEAT_PX).max(0.0)
        };
        let right = if interior_right {
            max_x + HUD_PLATE_JOIN_OVERLAP_PX
        } else {
            root_width - endcap_right + HUD_ENDCAP_SEAT_PX
        };
        let width = (right - left).max(0.0);

        for (frame, mut node) in &mut frames {
            if frame.section != section {
                continue;
            }
            node.left = Val::Px(left);
            node.width = Val::Px(width);
        }
    }
}
