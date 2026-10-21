//! Player dialogue approach and session open dispatch.

use bevy::prelude::*;

use crate::client::inventory_intent::InventoryIntentQueue;
use crate::client::{open_npc_trade_session, open_peer_unit_transfer};
use crate::ui::gameplay::InventoryUiState;
use crate::ui::gameplay::FloatingGameplayWindowRegistry;
use crate::ui::gameplay::dialogue::DialogueSessionState;
use crate::ui::gameplay::{
    UnitInteractionMenuState, build_interaction_menu_rows, is_player_to_player_trade_pair,
    unit_accepts_context_interaction_menu, uses_player_trade_only_menu,
};
use crate::client::TradeSessionState;
use crate::units::input::{MoveOrdersReport, SelectedUnits, issue_move_orders_to_selection};
use crate::world::{AttackTargetingPolicy, DialogueActionKind, is_unit_alive};
use crate::world::relationship::AuthoredRelationshipCatalog;
use crate::world::{
    DoodadCatalog, NavigationConfig, UnitCatalog, UnitId, WeaponCatalog, WorldData,
    is_player_controllable, unit_supports_dialogue, units_within_dialogue_range,
};

/// A unit approaching an NPC to perform a selected social action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingDialogueInteraction {
    pub actor_unit_id: UnitId,
    pub target_unit_id: UnitId,
    pub action: DialogueActionKind,
}

#[derive(Resource, Default, Debug)]
pub struct PendingDialogueInteractionState {
    pending: Option<PendingDialogueInteraction>,
    /// Dev: dedupe peer-trade trace lines to one log per user attempt.
    peer_trade_log_serial: u64,
    last_peer_trade_logged_serial: u64,
}

impl PendingDialogueInteractionState {
    pub fn next_peer_trade_log_serial(&mut self) -> u64 {
        self.peer_trade_log_serial += 1;
        self.peer_trade_log_serial
    }

    pub fn get(&self) -> Option<PendingDialogueInteraction> {
        self.pending
    }

    pub fn set(
        &mut self,
        actor_unit_id: UnitId,
        target_unit_id: UnitId,
        action: DialogueActionKind,
    ) {
        self.pending = Some(PendingDialogueInteraction {
            actor_unit_id,
            target_unit_id,
            action,
        });
    }

    pub fn clear(&mut self) {
        self.pending = None;
    }

    pub fn clear_for_unit(&mut self, unit_id: UnitId) {
        if self
            .pending
            .is_some_and(|pending| pending.actor_unit_id == unit_id)
        {
            self.pending = None;
        }
    }
}

pub fn supersede_pending_dialogue_for_selection(
    pending: &mut PendingDialogueInteractionState,
    selection: &SelectedUnits,
) {
    for unit_id in selection.iter() {
        pending.clear_for_unit(unit_id);
    }
}

fn pending_still_valid(
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    pending: PendingDialogueInteraction,
) -> bool {
    let actor = match world.get_unit(pending.actor_unit_id) {
        Some(record) => record,
        None => return false,
    };
    let target = match world.get_unit(pending.target_unit_id) {
        Some(record) => record,
        None => return false,
    };
    if !is_unit_alive(actor) || !is_unit_alive(target) {
        return false;
    }
    if is_player_to_player_trade_pair(world, pending.actor_unit_id, pending.target_unit_id) {
        return pending.action == DialogueActionKind::Trade
            && actor.current_space_id == target.current_space_id;
    }
    if !unit_supports_dialogue(target) {
        return false;
    }
    if actor.current_space_id != target.current_space_id {
        return false;
    }
    let standing = world.relationship_standing_store();
    crate::world::evaluate_dialogue_option(
        world,
        authored_relationships,
        standing,
        pending.actor_unit_id,
        pending.target_unit_id,
        pending.action,
    )
    .is_available()
}

fn apply_dialogue_action(
    dialogue: &mut DialogueSessionState,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
    action: DialogueActionKind,
) {
    dialogue.open_session(actor_unit_id, target_unit_id);
    dialogue.select_option(action, actor_unit_id, target_unit_id);
}

fn complete_social_action(
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    unit_catalog: &UnitCatalog,
    dialogue: &mut DialogueSessionState,
    trade: &mut TradeSessionState,
    inventory_ui: &mut InventoryUiState,
    queue: &mut InventoryIntentQueue,
    registry: &mut FloatingGameplayWindowRegistry,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
    action: DialogueActionKind,
) -> bool {
    if action == DialogueActionKind::Trade {
        let opened = if is_player_to_player_trade_pair(world, actor_unit_id, target_unit_id) {
            open_peer_unit_transfer(
                world,
                unit_catalog,
                trade,
                inventory_ui,
                queue,
                registry,
                actor_unit_id,
                target_unit_id,
            )
        } else {
            open_npc_trade_session(
                world,
                authored_relationships,
                trade,
                inventory_ui,
                queue,
                registry,
                actor_unit_id,
                target_unit_id,
            )
        };
        if opened {
            dialogue.close();
            return true;
        }
        return false;
    }
    apply_dialogue_action(dialogue, actor_unit_id, target_unit_id, action);
    true
}

pub fn try_open_unit_interaction_menu(
    menu: &mut UnitInteractionMenuState,
    dialogue: &mut DialogueSessionState,
    pending: &mut PendingDialogueInteractionState,
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
    screen_position: Vec2,
) -> bool {
    if !unit_accepts_context_interaction_menu(world, actor_unit_id, target_unit_id) {
        return false;
    }

    pending.clear_for_unit(actor_unit_id);
    dialogue.close();
    menu.open_at(actor_unit_id, target_unit_id, screen_position);
    true
}

pub fn dispatch_dialogue_action(
    world: &mut WorldData,
    dialogue: &mut DialogueSessionState,
    pending: &mut PendingDialogueInteractionState,
    trade: &mut TradeSessionState,
    inventory_ui: &mut InventoryUiState,
    queue: &mut InventoryIntentQueue,
    registry: &mut FloatingGameplayWindowRegistry,
    authored_relationships: &AuthoredRelationshipCatalog,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
    action: DialogueActionKind,
) -> DialogueDispatchOutcome {
    if is_player_to_player_trade_pair(world, actor_unit_id, target_unit_id)
        && action == DialogueActionKind::Trade
    {
        return dispatch_social_action_after_range_check(
            world,
            dialogue,
            pending,
            trade,
            inventory_ui,
            queue,
            registry,
            authored_relationships,
            unit_catalog,
            weapon_catalog,
            doodad_catalog,
            nav_config,
            actor_unit_id,
            target_unit_id,
            action,
        );
    }

    if uses_player_trade_only_menu(world, actor_unit_id, target_unit_id) {
        if action != DialogueActionKind::Trade {
            return DialogueDispatchOutcome::Ignored;
        }
        return dispatch_social_action_after_range_check(
            world,
            dialogue,
            pending,
            trade,
            inventory_ui,
            queue,
            registry,
            authored_relationships,
            unit_catalog,
            weapon_catalog,
            doodad_catalog,
            nav_config,
            actor_unit_id,
            target_unit_id,
            action,
        );
    }

    let target = world.get_unit(target_unit_id);
    if !target.is_some_and(unit_supports_dialogue) {
        return DialogueDispatchOutcome::Ignored;
    }

    let standing = world.relationship_standing_store();
    if !crate::world::evaluate_dialogue_option(
        world,
        authored_relationships,
        standing,
        actor_unit_id,
        target_unit_id,
        action,
    )
    .is_available()
    {
        return DialogueDispatchOutcome::Ignored;
    }

    dispatch_social_action_after_range_check(
        world,
        dialogue,
        pending,
        trade,
        inventory_ui,
        queue,
        registry,
        authored_relationships,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        actor_unit_id,
        target_unit_id,
        action,
    )
}

#[cfg(feature = "dev")]
fn log_peer_trade_attempt(
    pending: &mut PendingDialogueInteractionState,
    world: &WorldData,
    inventory_ui: &InventoryUiState,
    log_serial: u64,
    stage: &str,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) {
    if log_serial == 0 || pending.last_peer_trade_logged_serial == log_serial {
        return;
    }
    pending.last_peer_trade_logged_serial = log_serial;
    let actor_player = world
        .get_unit(actor_unit_id)
        .is_some_and(is_player_controllable);
    let target_player = world
        .get_unit(target_unit_id)
        .is_some_and(is_player_controllable);
    let in_range = units_within_dialogue_range(world, actor_unit_id, target_unit_id);
    info!(
        "peer_trade attempt={} stage={} actor={} target={} actor_player={} target_player={} in_range={} ui_open={} peer_bound={}",
        log_serial,
        stage,
        actor_unit_id.raw(),
        target_unit_id.raw(),
        actor_player,
        target_player,
        in_range,
        inventory_ui.open,
        inventory_ui.peer_unit_id.is_some(),
    );
}

fn dispatch_social_action_after_range_check(
    world: &mut WorldData,
    dialogue: &mut DialogueSessionState,
    pending: &mut PendingDialogueInteractionState,
    trade: &mut TradeSessionState,
    inventory_ui: &mut InventoryUiState,
    queue: &mut InventoryIntentQueue,
    registry: &mut FloatingGameplayWindowRegistry,
    authored_relationships: &AuthoredRelationshipCatalog,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
    action: DialogueActionKind,
) -> DialogueDispatchOutcome {
    let log_serial = if action == DialogueActionKind::Trade
        && is_player_to_player_trade_pair(world, actor_unit_id, target_unit_id)
    {
        pending.next_peer_trade_log_serial()
    } else {
        0
    };

    if units_within_dialogue_range(world, actor_unit_id, target_unit_id) {
        pending.clear_for_unit(actor_unit_id);
        if complete_social_action(
            world,
            authored_relationships,
            unit_catalog,
            dialogue,
            trade,
            inventory_ui,
            queue,
            registry,
            actor_unit_id,
            target_unit_id,
            action,
        ) {
            #[cfg(feature = "dev")]
            if log_serial != 0 {
                log_peer_trade_attempt(
                    pending,
                    world,
                    inventory_ui,
                    log_serial,
                    "opened_in_range",
                    actor_unit_id,
                    target_unit_id,
                );
            }
            return DialogueDispatchOutcome::Opened;
        }
        #[cfg(feature = "dev")]
        if log_serial != 0 {
            log_peer_trade_attempt(
                pending,
                world,
                inventory_ui,
                log_serial,
                "open_failed_in_range",
                actor_unit_id,
                target_unit_id,
            );
        }
        if inventory_ui.feedback_message.is_empty() {
            inventory_ui.feedback_message = "Cannot do that right now.".into();
        }
        return DialogueDispatchOutcome::Ignored;
    }

    pending.set(actor_unit_id, target_unit_id, action);
    #[cfg(feature = "dev")]
    if log_serial != 0 {
        log_peer_trade_attempt(
            pending,
            world,
            inventory_ui,
            log_serial,
            "deferred_approach",
            actor_unit_id,
            target_unit_id,
        );
    }
    let issued_approach = issue_approach_to_dialogue(
        world,
        unit_catalog,
        weapon_catalog,
        doodad_catalog,
        nav_config,
        actor_unit_id,
        target_unit_id,
    )
    .issued
        > 0;

    DialogueDispatchOutcome::Deferred { issued_approach }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueDispatchOutcome {
    Ignored,
    Opened,
    Deferred { issued_approach: bool },
}

fn issue_approach_to_dialogue(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    nav_config: &NavigationConfig,
    actor_unit_id: UnitId,
    target_unit_id: UnitId,
) -> MoveOrdersReport {
    let Some(target) = world.get_unit(target_unit_id) else {
        return MoveOrdersReport::default();
    };
    let target_position = target.placement.position;
    let mut actor_only = SelectedUnits::default();
    actor_only.set_single(actor_unit_id);
    issue_move_orders_to_selection(
        world,
        &actor_only,
        unit_catalog,
        weapon_catalog,
        &crate::world::ItemCatalog::default(),
        doodad_catalog,
        nav_config,
        target_position,
        AttackTargetingPolicy::default(),
        &[target_unit_id],
    )
}

pub fn try_complete_pending_dialogue_interaction(
    dialogue: &mut DialogueSessionState,
    pending: &mut PendingDialogueInteractionState,
    trade: &mut TradeSessionState,
    inventory_ui: &mut InventoryUiState,
    queue: &mut InventoryIntentQueue,
    registry: &mut FloatingGameplayWindowRegistry,
    world: &WorldData,
    authored_relationships: &AuthoredRelationshipCatalog,
    unit_catalog: &UnitCatalog,
) -> bool {
    let Some(pending_interaction) = pending.get() else {
        return false;
    };

    if !pending_still_valid(world, authored_relationships, pending_interaction) {
        #[cfg(feature = "dev")]
        if pending_interaction.action == DialogueActionKind::Trade
            && is_player_to_player_trade_pair(
                world,
                pending_interaction.actor_unit_id,
                pending_interaction.target_unit_id,
            )
        {
            log_peer_trade_attempt(
                pending,
                world,
                inventory_ui,
                pending.peer_trade_log_serial,
                "pending_dropped_still_invalid",
                pending_interaction.actor_unit_id,
                pending_interaction.target_unit_id,
            );
        }
        pending.clear();
        return false;
    }

    if !units_within_dialogue_range(
        world,
        pending_interaction.actor_unit_id,
        pending_interaction.target_unit_id,
    ) {
        return false;
    }

    let completed = complete_social_action(
        world,
        authored_relationships,
        unit_catalog,
        dialogue,
        trade,
        inventory_ui,
        queue,
        registry,
        pending_interaction.actor_unit_id,
        pending_interaction.target_unit_id,
        pending_interaction.action,
    );
    #[cfg(feature = "dev")]
    if pending_interaction.action == DialogueActionKind::Trade
        && is_player_to_player_trade_pair(
            world,
            pending_interaction.actor_unit_id,
            pending_interaction.target_unit_id,
        )
    {
        let stage = if completed {
            "completed_after_approach"
        } else {
            "complete_failed_after_approach"
        };
        log_peer_trade_attempt(
            pending,
            world,
            inventory_ui,
            pending.peer_trade_log_serial,
            stage,
            pending_interaction.actor_unit_id,
            pending_interaction.target_unit_id,
        );
    }
    pending.clear();
    completed
}

pub fn tick_pending_dialogue_interactions(
    mut dialogue: ResMut<DialogueSessionState>,
    mut pending: ResMut<PendingDialogueInteractionState>,
    mut trade: ResMut<TradeSessionState>,
    mut inventory_ui: ResMut<InventoryUiState>,
    mut queue: ResMut<InventoryIntentQueue>,
    mut registry: ResMut<FloatingGameplayWindowRegistry>,
    world: Res<WorldData>,
    authored_relationships: Res<AuthoredRelationshipCatalog>,
    unit_catalog: Res<UnitCatalog>,
) {
    try_complete_pending_dialogue_interaction(
        &mut dialogue,
        &mut pending,
        &mut trade,
        &mut inventory_ui,
        &mut queue,
        &mut registry,
        &world,
        &authored_relationships,
        &unit_catalog,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::inventory_intent::InventoryIntentQueue;
    use crate::ui::gameplay::{
        FloatingGameplayWindowRegistry, InventoryUiState, is_player_to_player_trade_pair,
    };
    use crate::world::relationship::AuthoredRelationshipCatalog;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, InventoryCatalogCtx,
        InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, LocalPosition, UnitCatalog,
        UnitDefinitionId, UnitOwnership, UnitSource, WorldData, WorldPosition,
        create_unit_with_inventory, starter_inventory_profile_definitions,
        starter_item_category_definitions, starter_item_definitions, starter_unit_definitions,
    };
    use bevy::prelude::Vec3;

    fn two_player_units_far_apart() -> (WorldData, UnitId, UnitId) {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let actor = create_unit_with_inventory(
            &unit_catalog,
            &crate::world::AppearanceProfileCatalog::empty(),
            &mut world,
            &UnitDefinitionId::new("bandit"),
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(0.0, 0.0, 0.0)),
            ),
            UnitSource::Authored,
            UnitOwnership::player_default(),
            &ctx,
        )
        .unwrap()
        .id;
        let target = create_unit_with_inventory(
            &unit_catalog,
            &crate::world::AppearanceProfileCatalog::empty(),
            &mut world,
            &UnitDefinitionId::new("bandit"),
            WorldPosition::new(
                ChunkCoord::new(0, 0),
                LocalPosition::new(Vec3::new(40.0, 0.0, 0.0)),
            ),
            UnitSource::Authored,
            UnitOwnership::player_default(),
            &ctx,
        )
        .unwrap()
        .id;
        (world, actor, target)
    }

    #[test]
    fn p2p_trade_pending_stays_valid_while_out_of_dialogue_range() {
        let (world, actor, target) = two_player_units_far_apart();
        assert!(is_player_to_player_trade_pair(&world, actor, target));
        assert!(
            !units_within_dialogue_range(&world, actor, target),
            "fixture should start out of range"
        );
        let pending = PendingDialogueInteraction {
            actor_unit_id: actor,
            target_unit_id: target,
            action: DialogueActionKind::Trade,
        };
        assert!(pending_still_valid(
            &world,
            &AuthoredRelationshipCatalog::default(),
            pending,
        ));
    }

    #[test]
    fn p2p_trade_pending_completes_after_approach_when_in_range() {
        let (mut world, actor, target) = two_player_units_far_apart();
        let mut pending_state = PendingDialogueInteractionState::default();
        pending_state.set(actor, target, DialogueActionKind::Trade);
        pending_state.next_peer_trade_log_serial();

        let mut dialogue = DialogueSessionState::default();
        let mut trade = TradeSessionState::default();
        let mut inventory_ui = InventoryUiState::default();
        let mut queue = InventoryIntentQueue::default();
        let mut registry = FloatingGameplayWindowRegistry::default();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();

        assert!(!try_complete_pending_dialogue_interaction(
            &mut dialogue,
            &mut pending_state,
            &mut trade,
            &mut inventory_ui,
            &mut queue,
            &mut registry,
            &world,
            &AuthoredRelationshipCatalog::default(),
            &unit_catalog,
        ));
        assert!(pending_state.get().is_some());

        let target_position = world.get_unit(target).unwrap().placement.position;
        world
            .relocate_unit(actor, target_position)
            .expect("move actor into dialogue range");

        assert!(try_complete_pending_dialogue_interaction(
            &mut dialogue,
            &mut pending_state,
            &mut trade,
            &mut inventory_ui,
            &mut queue,
            &mut registry,
            &world,
            &AuthoredRelationshipCatalog::default(),
            &unit_catalog,
        ));
        assert!(pending_state.get().is_none());
        assert!(inventory_ui.open);
        assert_eq!(inventory_ui.peer_unit_id, Some(target));
    }
}
