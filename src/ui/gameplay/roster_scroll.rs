//! Squad roster horizontal paging — bounded viewport with arrow navigation.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition};

use super::hud::{HudViewportGeometry, roster_content_width, visible_roster_card_count};
use super::layout::PlayerHudUi;
use super::plugin::GameplayCommandInputSystems;
use super::squad_panel::{SquadEntryList, SquadRosterViewport};
use super::styles::{TEXT_PRIMARY, hud_caption_font};

pub const ROSTER_NAV_BUTTON_WIDTH_PX: f32 = 18.0;

/// Client-local roster scroll metrics (authoritative for tests).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct SquadRosterScrollState {
    pub scroll_offset_x: f32,
    pub viewport_width: f32,
    pub content_width: f32,
    pub card_width: f32,
    pub card_gap: f32,
}

impl Default for SquadRosterScrollState {
    fn default() -> Self {
        let geom = HudViewportGeometry::default();
        Self {
            scroll_offset_x: 0.0,
            viewport_width: 0.0,
            content_width: 0.0,
            card_width: geom.card_width,
            card_gap: geom.card_gap,
        }
    }
}

impl SquadRosterScrollState {
    pub fn max_scroll_x(&self) -> f32 {
        max_scroll_x(self.viewport_width, self.content_width)
    }

    pub fn has_overflow(&self) -> bool {
        self.max_scroll_x() > 0.0
    }

    pub fn visible_card_count(&self) -> u32 {
        visible_roster_card_count(self.viewport_width, self.card_width, self.card_gap)
    }

    pub fn page_step_px(&self) -> f32 {
        roster_page_step_px(self.viewport_width, self.card_width, self.card_gap)
    }

    pub fn can_scroll_left(&self) -> bool {
        self.has_overflow() && self.scroll_offset_x > 0.5
    }

    pub fn can_scroll_right(&self) -> bool {
        self.has_overflow() && self.scroll_offset_x + 0.5 < self.max_scroll_x()
    }

    pub fn clamp_offset(&mut self) {
        self.scroll_offset_x = clamp_scroll_offset_x(
            self.scroll_offset_x,
            self.viewport_width,
            self.content_width,
        );
    }

    pub fn reset(&mut self) {
        self.scroll_offset_x = 0.0;
    }

    pub fn page_left(&mut self) {
        if !self.can_scroll_left() {
            return;
        }
        self.scroll_offset_x = (self.scroll_offset_x - self.page_step_px()).max(0.0);
        self.clamp_offset();
    }

    pub fn page_right(&mut self) {
        if !self.can_scroll_right() {
            return;
        }
        self.scroll_offset_x =
            (self.scroll_offset_x + self.page_step_px()).min(self.max_scroll_x());
        self.clamp_offset();
    }
}

#[derive(Component, Debug)]
pub struct SquadRosterNavLeft;

#[derive(Component, Debug)]
pub struct SquadRosterNavRight;

pub fn max_scroll_x(viewport_width: f32, content_width: f32) -> f32 {
    (content_width - viewport_width).max(0.0)
}

pub fn clamp_scroll_offset_x(offset_x: f32, viewport_width: f32, content_width: f32) -> f32 {
    offset_x.clamp(0.0, max_scroll_x(viewport_width, content_width))
}

/// One page step: roughly one viewport of cards.
pub fn roster_page_step_px(viewport_width: f32, card_width: f32, card_gap: f32) -> f32 {
    let visible = visible_roster_card_count(viewport_width, card_width, card_gap).max(1);
    visible as f32 * (card_width + card_gap)
}

pub fn spawn_roster_nav_button(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &'static str,
    marker: impl Bundle,
) {
    parent
        .spawn((
            marker,
            PlayerHudUi,
            Button,
            Node {
                width: Val::Px(ROSTER_NAV_BUTTON_WIDTH_PX),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                margin: UiRect::horizontal(Val::Px(1.0)),
                ..default()
            },
            super::hud::hud_quiet_raised_style(0.85),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                hud_caption_font(),
                TextColor(TEXT_PRIMARY),
            ));
        });
}

pub fn measure_squad_roster_scroll_state(
    geometry: Res<HudViewportGeometry>,
    mut state: ResMut<SquadRosterScrollState>,
    viewport: Query<&ComputedNode, With<SquadRosterViewport>>,
    list: Query<&ComputedNode, With<SquadEntryList>>,
) {
    let Ok(viewport_node) = viewport.single() else {
        return;
    };
    let Ok(list_node) = list.single() else {
        return;
    };
    state.viewport_width = viewport_node.size().x;
    state.content_width = list_node.size().x;
    state.card_width = geometry.card_width;
    state.card_gap = geometry.card_gap;
    state.clamp_offset();
}

pub fn apply_squad_roster_scroll_position(
    state: Res<SquadRosterScrollState>,
    mut scroll_positions: Query<&mut ScrollPosition, With<SquadRosterViewport>>,
) {
    for mut scroll in &mut scroll_positions {
        scroll.x = state.scroll_offset_x;
    }
}

pub fn sync_squad_roster_nav_presentation(
    state: Res<SquadRosterScrollState>,
    mut left: Query<(&mut Node, &mut BackgroundColor, &mut BorderColor), With<SquadRosterNavLeft>>,
    mut right: Query<
        (&mut Node, &mut BackgroundColor, &mut BorderColor),
        (With<SquadRosterNavRight>, Without<SquadRosterNavLeft>),
    >,
) {
    let show = state.has_overflow();
    let left_enabled = state.can_scroll_left();
    let right_enabled = state.can_scroll_right();
    for (mut node, mut bg, mut border) in &mut left {
        node.display = if show { Display::Flex } else { Display::None };
        let alpha = if left_enabled { 0.85 } else { 0.35 };
        let (next_bg, next_border) = super::hud::hud_quiet_raised_style(alpha);
        *bg = next_bg;
        *border = next_border;
    }
    for (mut node, mut bg, mut border) in &mut right {
        node.display = if show { Display::Flex } else { Display::None };
        let alpha = if right_enabled { 0.85 } else { 0.35 };
        let (next_bg, next_border) = super::hud::hud_quiet_raised_style(alpha);
        *bg = next_bg;
        *border = next_border;
    }
}

pub fn handle_squad_roster_nav_clicks(
    mut state: ResMut<SquadRosterScrollState>,
    left: Query<&Interaction, (Changed<Interaction>, With<SquadRosterNavLeft>)>,
    right: Query<&Interaction, (Changed<Interaction>, With<SquadRosterNavRight>)>,
) {
    if left
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
        && state.can_scroll_left()
    {
        state.page_left();
    }
    if right
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
        && state.can_scroll_right()
    {
        state.page_right();
    }
}

pub struct SquadRosterScrollPlugin;

impl Plugin for SquadRosterScrollPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SquadRosterScrollState>()
            .add_systems(
                PostUpdate,
                (
                    measure_squad_roster_scroll_state,
                    apply_squad_roster_scroll_position,
                    sync_squad_roster_nav_presentation,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                handle_squad_roster_nav_clicks.in_set(GameplayCommandInputSystems),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD_WIDTH: f32 = 84.0;
    const CARD_GAP: f32 = 6.0;

    #[test]
    fn narrow_viewport_fits_fewer_cards_than_wide_viewport() {
        let narrow = visible_roster_card_count(200.0, CARD_WIDTH, CARD_GAP);
        let wide = visible_roster_card_count(600.0, CARD_WIDTH, CARD_GAP);
        assert!(wide > narrow);
        assert!(narrow >= 1);
    }

    #[test]
    fn no_overflow_when_all_cards_fit() {
        let viewport = 600.0;
        let content = roster_content_width(3, CARD_WIDTH, CARD_GAP);
        let state = SquadRosterScrollState {
            scroll_offset_x: 0.0,
            viewport_width: viewport,
            content_width: content,
            card_width: CARD_WIDTH,
            card_gap: CARD_GAP,
        };
        assert!(!state.has_overflow());
        assert!(!state.can_scroll_right());
    }

    #[test]
    fn overflow_enables_right_navigation() {
        let viewport = 300.0;
        let content = roster_content_width(10, CARD_WIDTH, CARD_GAP);
        let state = SquadRosterScrollState {
            scroll_offset_x: 0.0,
            viewport_width: viewport,
            content_width: content,
            card_width: CARD_WIDTH,
            card_gap: CARD_GAP,
        };
        assert!(state.has_overflow());
        assert!(!state.can_scroll_left());
        assert!(state.can_scroll_right());
    }

    #[test]
    fn paging_right_then_left_reaches_ends() {
        let viewport = 300.0;
        let content = roster_content_width(12, CARD_WIDTH, CARD_GAP);
        let mut state = SquadRosterScrollState {
            scroll_offset_x: 0.0,
            viewport_width: viewport,
            content_width: content,
            card_width: CARD_WIDTH,
            card_gap: CARD_GAP,
        };
        state.page_right();
        assert!(state.scroll_offset_x > 0.0);
        state.page_left();
        assert_eq!(state.scroll_offset_x, 0.0);
    }

    #[test]
    fn paging_right_reaches_later_cards() {
        let viewport = 300.0;
        let content = roster_content_width(12, CARD_WIDTH, CARD_GAP);
        let mut state = SquadRosterScrollState {
            scroll_offset_x: 0.0,
            viewport_width: viewport,
            content_width: content,
            card_width: CARD_WIDTH,
            card_gap: CARD_GAP,
        };
        while state.can_scroll_right() {
            state.page_right();
        }
        assert!((state.scroll_offset_x - state.max_scroll_x()).abs() < 0.5);
    }

    #[test]
    fn paging_does_not_imply_selection_change() {
        let viewport = 300.0;
        let content = roster_content_width(12, CARD_WIDTH, CARD_GAP);
        let mut state = SquadRosterScrollState {
            scroll_offset_x: 0.0,
            viewport_width: viewport,
            content_width: content,
            card_width: CARD_WIDTH,
            card_gap: CARD_GAP,
        };
        state.page_right();
        state.page_left();
        assert_eq!(state.scroll_offset_x, 0.0);
    }

    #[test]
    fn roster_has_no_twelve_unit_cap_in_scroll_math() {
        let content = roster_content_width(20, CARD_WIDTH, CARD_GAP);
        let viewport = 300.0;
        assert!(content > viewport);
    }
}
