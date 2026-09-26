//! Unit interaction menu keyboard and pointer dismissal.

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::client::{PendingDialogueInteractionState, dispatch_dialogue_action};
use crate::ui::gameplay::build_mode::BuildModeState;
use crate::ui::gameplay::dialogue::DialogueSessionState;
use crate::world::relationship::AuthoredRelationshipCatalog;
use crate::world::{
    AttackTargetingPolicy, DoodadCatalog, NavigationConfig, UnitCatalog, WeaponCatalog, WorldData,
    evaluate_dialogue_option,
};

use super::panel::{
    UnitInteractionMenuBackdrop, UnitInteractionMenuOptionButton, UnitInteractionMenuRoot,
};
use super::state::UnitInteractionMenuState;

pub fn collect_unit_interaction_menu_keyboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<UnitInteractionMenuState>,
    build_mode: Res<BuildModeState>,
    menu_block: Option<Res<crate::menu::MenuInputBlock>>,
    #[cfg(feature = "dev")] dev_state: Option<Res<crate::dev::DevModeState>>,
) {
    if menu_block.is_some_and(|block| block.blocks()) {
        return;
    }
    if build_mode.search_focused {
        return;
    }
    #[cfg(feature = "dev")]
    if dev_state.is_some_and(|state| state.has_text_focus()) {
        return;
    }
    if !menu.open {
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        menu.close();
    }
}

pub fn handle_unit_interaction_menu_backdrop_click(
    mut menu: ResMut<UnitInteractionMenuState>,
    backdrop: Query<&Interaction, With<UnitInteractionMenuBackdrop>>,
) {
    if !menu.open {
        return;
    }
    if backdrop.iter().any(|interaction| *interaction == Interaction::Pressed) {
        menu.close();
    }
}

pub fn handle_unit_interaction_menu_option_clicks(
    mut menu: ResMut<UnitInteractionMenuState>,
    mut dialogue: ResMut<DialogueSessionState>,
    mut pending: ResMut<PendingDialogueInteractionState>,
    mut world: ResMut<WorldData>,
    authored_relationships: Res<AuthoredRelationshipCatalog>,
    unit_catalog: Res<UnitCatalog>,
    weapon_catalog: Res<WeaponCatalog>,
    doodad_catalog: Res<DoodadCatalog>,
    nav_config: Res<NavigationConfig>,
    buttons: Query<
        (&Interaction, &UnitInteractionMenuOptionButton),
        Without<UnitInteractionMenuBackdrop>,
    >,
) {
    if !menu.open {
        return;
    }
    let actor = match menu.actor_unit_id {
        Some(id) => id,
        None => return,
    };
    let target = match menu.target_unit_id {
        Some(id) => id,
        None => return,
    };
    let standing = world.relationship_standing_store();

    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if !evaluate_dialogue_option(
            &world,
            &authored_relationships,
            standing,
            actor,
            target,
            button.0,
        )
        .is_available()
        {
            continue;
        }
        let kind = button.0;
        menu.close();
        dispatch_dialogue_action(
            &mut world,
            &mut dialogue,
            &mut pending,
            &authored_relationships,
            &unit_catalog,
            &weapon_catalog,
            &doodad_catalog,
            &nav_config,
            actor,
            target,
            kind,
        );
        return;
    }
}

pub fn dismiss_unit_interaction_menu_on_outside_click(
    mut menu: ResMut<UnitInteractionMenuState>,
    mouse: Res<ButtonInput<MouseButton>>,
    menu_hits: Query<&Interaction, With<UnitInteractionMenuRoot>>,
    option_hits: Query<&Interaction, With<UnitInteractionMenuOptionButton>>,
) {
    if !menu.open || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let ui_hit = menu_hits
        .iter()
        .chain(option_hits.iter())
        .any(|interaction| *interaction != Interaction::None);
    if !ui_hit {
        menu.close();
    }
}

pub fn reconcile_unit_interaction_menu(
    mut menu: ResMut<UnitInteractionMenuState>,
    world: Res<WorldData>,
    authored_relationships: Res<AuthoredRelationshipCatalog>,
    weapon_catalog: Res<WeaponCatalog>,
    unit_catalog: Res<UnitCatalog>,
    item_catalog: Res<crate::world::ItemCatalog>,
) {
    if !menu.open {
        return;
    }
    let actor = match menu.actor_unit_id {
        Some(id) => id,
        None => {
            menu.close();
            return;
        }
    };
    let target = match menu.target_unit_id {
        Some(id) => id,
        None => {
            menu.close();
            return;
        }
    };

    let target_alive = world.get_unit(target).is_some_and(crate::world::is_unit_alive);
    let actor_alive = world.get_unit(actor).is_some_and(crate::world::is_unit_alive);
    if !target_alive || !actor_alive {
        menu.close();
        return;
    }

    if crate::world::is_valid_autonomous_attack_target(
        &world,
        &authored_relationships,
        actor,
        target,
        &weapon_catalog,
        &unit_catalog,
        &item_catalog,
        AttackTargetingPolicy::default(),
    ) {
        menu.close();
        return;
    }

    let rows = super::content::build_interaction_menu_rows(
        &world,
        &authored_relationships,
        world.relationship_standing_store(),
        actor,
        target,
    );
    if rows.is_empty() {
        menu.close();
    }
}
