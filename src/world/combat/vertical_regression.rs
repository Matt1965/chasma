//! Phase 7 vertical regression — end-to-end relationship → perception → desire → combat path.
//!
//! Proves the implemented authority chain in one place; component-level tests remain in their
//! owning modules.

#[cfg(test)]
mod tests {
    use super::super::engagement::scan_attack_move_target;
    use super::super::test_support::phase6_authored_catalog;
    use super::super::{
        HOSTILE_RELATIONSHIP_THRESHOLD, classify_unit_target, evaluate_autonomous_desire,
        find_auto_acquire_target, step_combat_ai_acquisition, validate_explicit_attack_target,
        validate_mechanical_attack_target,
    };
    use crate::world::combat::{
        CombatAiScanState, CombatAiSettings, CombatAiTraceOutcome, apply_attributed_combat_damage,
    };
    use crate::world::ownership::UnitOwnership;
    use crate::world::perception::perceived_units;
    use crate::world::relationship::{
        FactionId, effective_relationship_for_records, explain_relationship_for_records,
    };
    use crate::world::unit::{CombatState, UnitId, UnitOrder};
    use crate::world::{
        AttackTargetingPolicy, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog,
        Heightfield, InteractionType, LocalPosition, NavigationConfig, UnitCatalog,
        UnitDefinitionId, UnitSource, WeaponCatalog, WorldData, WorldPosition,
        create_unit_with_ownership, issue_unit_order, starter_unit_definitions,
        starter_weapon_definitions,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn catalogs() -> (UnitCatalog, WeaponCatalog) {
        (
            UnitCatalog::from_definitions(starter_unit_definitions()).unwrap(),
            WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap(),
        )
    }

    fn policy() -> AttackTargetingPolicy {
        AttackTargetingPolicy::default()
    }

    fn authored() -> crate::world::AuthoredRelationshipCatalog {
        phase6_authored_catalog()
    }

    fn patch_player_faction(world: &mut WorldData, unit_id: UnitId) {
        let mut record = world.remove_unit_by_id(unit_id).expect("unit exists");
        record.faction_id = FactionId::new("player");
        let chunk = ChunkId::new(record.placement.position.chunk);
        world.insert_unit(chunk, record).unwrap();
    }

    fn spawn_wild_wolf(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        position: WorldPosition,
    ) -> UnitId {
        create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            position,
            UnitSource::Authored,
            UnitOwnership::wildlife(),
        )
        .unwrap()
        .id
    }

    fn spawn_player_unit(
        world: &mut WorldData,
        catalog: &UnitCatalog,
        position: WorldPosition,
    ) -> UnitId {
        let id = create_unit_with_ownership(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            position,
            UnitSource::Authored,
            UnitOwnership::player_default(),
        )
        .unwrap()
        .id;
        patch_player_faction(world, id);
        id
    }

    fn fast_ai_settings() -> CombatAiSettings {
        CombatAiSettings {
            scan_interval_seconds: 0.0,
            ..CombatAiSettings::default()
        }
    }

    /// Wild observer → player target through perception, relationship, desire, and Attack order.
    #[test]
    fn phase7_wild_vertical_path_from_perception_to_attack_order() {
        let (catalog, weapons) = catalogs();
        let authored = authored();
        let mut world = flat_world();
        let wild = spawn_wild_wolf(&mut world, &catalog, pos(0.0, 0.0));
        let player_far = spawn_player_unit(&mut world, &catalog, pos(40.0, 0.0));

        assert!(
            perceived_units(&world, &catalog, wild).is_empty(),
            "player outside sight range must not be a perception candidate"
        );
        assert!(
            find_auto_acquire_target(&world, wild, &catalog, &weapons, policy(), &authored)
                .is_none()
        );

        world
            .relocate_unit(player_far, pos(8.0, 0.0))
            .expect("move player into sight");

        assert!(
            perceived_units(&world, &catalog, wild)
                .iter()
                .any(|id| *id == player_far),
            "player inside sight range must be perceived"
        );

        let wild_record = world.get_unit(wild).unwrap().clone();
        let player_record = world.get_unit(player_far).unwrap().clone();
        assert_eq!(
            effective_relationship_for_records(
                &authored,
                world.relationship_standing_store(),
                &wild_record,
                &player_record,
            ),
            -300
        );
        assert_eq!(
            effective_relationship_for_records(
                &authored,
                world.relationship_standing_store(),
                &player_record,
                &wild_record,
            ),
            0
        );

        let explanation = explain_relationship_for_records(
            &authored,
            world.relationship_standing_store(),
            &wild_record,
            &player_record,
        );
        let contribution_sum: i32 = explanation.contributions.iter().map(|c| c.value).sum();
        assert_eq!(contribution_sum, explanation.total);
        assert_eq!(explanation.total, -300);

        let desire =
            evaluate_autonomous_desire(&world, &authored, &wild_record, &player_record, policy());
        assert_eq!(desire.effective_relationship, -300);
        assert_eq!(desire.threshold, HOSTILE_RELATIONSHIP_THRESHOLD);
        assert!(desire.wants_attack);

        let reverse =
            evaluate_autonomous_desire(&world, &authored, &player_record, &wild_record, policy());
        assert_eq!(reverse.effective_relationship, 0);
        assert!(!reverse.wants_attack);

        let mut scan = CombatAiScanState::default();
        let report = step_combat_ai_acquisition(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
            &authored,
            &fast_ai_settings(),
            &mut scan,
            1.0,
        );
        assert!(report.traces.iter().any(|trace| trace.outcome
            == CombatAiTraceOutcome::AiTargetAcquired
            && trace.target == Some(player_far)));
        assert!(matches!(
            world.get_unit(wild).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == player_far
        ));
    }

    #[test]
    fn phase7_player_intent_separation_at_zero_relationship() {
        let (catalog, weapons) = catalogs();
        let authored = authored();
        let mut world = flat_world();
        let player = spawn_player_unit(&mut world, &catalog, pos(0.0, 0.0));
        let wild = spawn_wild_wolf(&mut world, &catalog, pos(8.0, 0.0));

        assert!(
            validate_mechanical_attack_target(&world, player, wild, &weapons, &catalog, policy())
                .is_ok()
        );
        assert!(
            validate_explicit_attack_target(&world, player, wild, &weapons, &catalog, policy())
                .is_ok()
        );
        assert!(
            scan_attack_move_target(&world, player, &catalog, &weapons, policy(), &authored)
                .is_none()
        );
        assert_ne!(
            classify_unit_target(
                &world,
                &authored,
                player,
                wild,
                &weapons,
                &catalog,
                policy(),
            ),
            InteractionType::AttackableUnit
        );
    }

    #[test]
    fn phase7_standing_can_block_vertical_proactive_acquire() {
        let (catalog, weapons) = catalogs();
        let authored = authored();
        let mut world = flat_world();
        let wild = spawn_wild_wolf(&mut world, &catalog, pos(0.0, 0.0));
        let player = spawn_player_unit(&mut world, &catalog, pos(8.0, 0.0));
        world.relationship_standing_store_mut().apply_delta(
            crate::world::RelationshipFacet::Individual(wild),
            crate::world::RelationshipFacet::Individual(player),
            250,
        );

        let wild_record = world.get_unit(wild).unwrap().clone();
        let player_record = world.get_unit(player).unwrap().clone();
        let desire =
            evaluate_autonomous_desire(&world, &authored, &wild_record, &player_record, policy());
        assert_eq!(desire.effective_relationship, -50);
        assert!(!desire.wants_attack);
        assert!(
            find_auto_acquire_target(&world, wild, &catalog, &weapons, policy(), &authored)
                .is_none()
        );
    }

    #[test]
    fn phase7_retaliation_without_proactive_hostility() {
        let (catalog, weapons) = catalogs();
        let authored = authored();
        let mut world = flat_world();
        let player = spawn_player_unit(&mut world, &catalog, pos(0.0, 0.0));
        let neutral = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(1.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::neutral(),
        )
        .unwrap()
        .id;

        let neutral_record = world.get_unit(neutral).unwrap().clone();
        let player_record = world.get_unit(player).unwrap().clone();
        assert!(
            !evaluate_autonomous_desire(
                &world,
                &authored,
                &neutral_record,
                &player_record,
                policy(),
            )
            .wants_attack
        );

        apply_attributed_combat_damage(
            &mut world,
            neutral,
            player,
            1,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            policy(),
        )
        .unwrap();
        assert!(matches!(
            world.get_unit(neutral).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == player
        ));
    }

    #[test]
    fn phase7_active_combat_continues_when_target_leaves_sight() {
        use crate::world::combat::step_all_combat_engagement;

        let (catalog, weapons) = catalogs();
        let authored = authored();
        let mut world = flat_world();
        let wild = spawn_wild_wolf(&mut world, &catalog, pos(0.0, 0.0));
        let player = spawn_player_unit(&mut world, &catalog, pos(6.0, 0.0));

        issue_unit_order(
            &mut world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &NavigationConfig::default(),
            wild,
            UnitOrder::Attack { target: player },
            policy(),
        )
        .unwrap();
        step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            crate::world::default_passability(),
            &NavigationConfig::default(),
            policy(),
            &authored,
            &mut crate::world::CombatStrikeReport::default(),
        );
        assert!(matches!(
            world.get_unit(wild).unwrap().combat_state,
            CombatState::Attacking { .. } | CombatState::Chasing { .. }
        ));

        world
            .relocate_unit(player, pos(40.0, 0.0))
            .expect("move target beyond sight");
        assert!(
            !perceived_units(&world, &catalog, wild)
                .iter()
                .any(|id| *id == player)
        );
        step_all_combat_engagement(
            &mut world,
            &catalog,
            &weapons,
            crate::world::default_passability(),
            &NavigationConfig::default(),
            policy(),
            &authored,
            &mut crate::world::CombatStrikeReport::default(),
        );
        assert!(matches!(
            world.get_unit(wild).unwrap().combat_state,
            CombatState::Attacking { target } | CombatState::Chasing { target } if target == player
        ));
    }

    #[test]
    fn phase7_exact_threshold_inclusive_and_affiliation_does_not_drive_desire() {
        use crate::world::relationship::{
            AuthoredFacetKey, AuthoredRelationshipCatalog, DirectedRelationshipEdgeKey,
        };

        let at_threshold = AuthoredRelationshipCatalog::from_edges([(
            DirectedRelationshipEdgeKey::new(
                AuthoredFacetKey::Faction(FactionId::new("wild")),
                AuthoredFacetKey::Faction(FactionId::new("player")),
            ),
            -100,
        )])
        .expect("valid edge");
        let above_threshold = AuthoredRelationshipCatalog::from_edges([(
            DirectedRelationshipEdgeKey::new(
                AuthoredFacetKey::Faction(FactionId::new("wild")),
                AuthoredFacetKey::Faction(FactionId::new("player")),
            ),
            -99,
        )])
        .expect("valid edge");

        let (catalog, _) = catalogs();
        let mut world = flat_world();
        let wild = spawn_wild_wolf(&mut world, &catalog, pos(0.0, 0.0));
        let player = spawn_player_unit(&mut world, &catalog, pos(1.0, 0.0));
        let wild_record = world.get_unit(wild).unwrap().clone();
        let player_record = world.get_unit(player).unwrap().clone();

        assert!(
            evaluate_autonomous_desire(
                &world,
                &at_threshold,
                &wild_record,
                &player_record,
                policy()
            )
            .wants_attack
        );
        assert!(
            !evaluate_autonomous_desire(
                &world,
                &above_threshold,
                &wild_record,
                &player_record,
                policy(),
            )
            .wants_attack
        );

        let hostile = create_unit_with_ownership(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(2.0, 0.0),
            UnitSource::Authored,
            UnitOwnership::hostile(),
        )
        .unwrap()
        .id;
        let hostile_record = world.get_unit(hostile).unwrap().clone();
        assert!(
            !evaluate_autonomous_desire(
                &world,
                &AuthoredRelationshipCatalog::default(),
                &hostile_record,
                &player_record,
                policy(),
            )
            .wants_attack
        );
    }
}
