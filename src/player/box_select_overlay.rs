//! Screen-space marquee rectangle while box-selecting (ADR-034 U9, U12 polish).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::units::input::{normalized_screen_rect, BoxSelectDrag};

/// Root UI node for the drag-selection rectangle.
#[derive(Component, Debug)]
pub struct BoxSelectOverlay;

const BOX_FILL: Color = Color::srgba(0.25, 0.9, 0.35, 0.12);
const BOX_BORDER: Color = Color::srgba(0.35, 1.0, 0.45, 0.95);

/// Spawn a hidden overlay node once at startup.
pub fn setup_box_select_overlay(mut commands: Commands) {
    commands.spawn((
        BoxSelectOverlay,
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(BOX_FILL),
        BorderColor::all(BOX_BORDER),
        FocusPolicy::Pass,
        ZIndex(200),
    ));
}

/// Show and resize the overlay while an active box drag exceeds the click threshold.
pub fn sync_box_select_overlay(
    box_drag: Res<BoxSelectDrag>,
    mut overlay: Query<(&mut Node, &mut Visibility), With<BoxSelectOverlay>>,
) {
    let Ok((mut node, mut visibility)) = overlay.single_mut() else {
        return;
    };

    if !box_drag.is_box_drag() {
        *visibility = Visibility::Hidden;
        node.display = Display::None;
        return;
    }

    let (min, max) = normalized_screen_rect(box_drag.start, box_drag.current);
    let width = (max.x - min.x).max(1.0);
    let height = (max.y - min.y).max(1.0);

    node.display = Display::Flex;
    node.left = Val::Px(min.x);
    node.top = Val::Px(min.y);
    node.width = Val::Px(width);
    node.height = Val::Px(height);
    *visibility = Visibility::Visible;
}
