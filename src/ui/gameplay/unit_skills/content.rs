//! Read-only unit skills / stats presentation model (player U screen).

use crate::ui::gameplay::combat_display::{
    append_combat_state_lines, append_weapon_hud_lines, combat_target_id, weapon_display_for_unit,
};
use crate::ui::gameplay::selected_unit_panel::unit_state_label;
use crate::world::{
    NutritionProfile, UnitCatalog, UnitId, UnitRecord, WeaponCatalog, WorkSkillCatalog, WorldData,
    evaluate_hunger_stage, hunger_stage_label, work_skill_value,
};

/// One labeled stat row in the skills panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitSkillsStatLine {
    pub label: String,
    pub value: String,
}

/// Grouped section (Attributes, Combat, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitSkillsSection {
    pub title: String,
    pub lines: Vec<UnitSkillsStatLine>,
}

/// Full panel snapshot for one unit (read-only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitSkillsPanelSnapshot {
    pub unit_id: UnitId,
    pub title: String,
    pub sections: Vec<UnitSkillsSection>,
}

pub fn build_unit_skills_snapshot(
    unit_id: UnitId,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    work_skill_catalog: &WorkSkillCatalog,
) -> Option<UnitSkillsPanelSnapshot> {
    let record = world.get_unit(unit_id)?;
    let def = unit_catalog.get(&record.definition_id)?;
    Some(build_snapshot_from_record(
        unit_id,
        record,
        def,
        world,
        unit_catalog,
        weapon_catalog,
        work_skill_catalog,
    ))
}

fn build_snapshot_from_record(
    unit_id: UnitId,
    record: &UnitRecord,
    def: &crate::world::UnitDefinition,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    work_skill_catalog: &WorkSkillCatalog,
) -> UnitSkillsPanelSnapshot {
    let mut sections = Vec::new();

    sections.push(UnitSkillsSection {
        title: "Overview".into(),
        lines: vec![
            line("Faction", def.faction_tag.clone()),
            line("Level", def.level.to_string()),
            line("Tier", def.tier.clone()),
            line(
                "HP",
                format!("{}/{}", record.vitals.current_hp, record.vitals.max_hp),
            ),
            line("State", unit_state_label(&record.state).to_string()),
            line("Combat posture", record.combat_state.label().to_string()),
        ],
    });

    sections.push(UnitSkillsSection {
        title: "Attributes".into(),
        lines: vec![
            line("Strength", def.strength.to_string()),
            line("Dexterity", def.dexterity.to_string()),
            line("Constitution", def.constitution.to_string()),
            line("Agility", def.agility.to_string()),
            line("Charisma", def.charisma.to_string()),
            line("Intelligence", def.intelligence.to_string()),
            line("Power rating", format!("{:.1}", def.power_rating)),
        ],
    });

    sections.push(UnitSkillsSection {
        title: "Physical".into(),
        lines: vec![
            line("Move speed", format!("{:.1} m/s", def.move_speed_mps)),
            line(
                "Collision radius",
                format!("{:.2} m", def.collision_radius_meters),
            ),
            line("Max slope", format!("{:.0}°", def.max_slope_degrees)),
            line("Sight range", format!("{:.0} m", def.sight_range_meters)),
            line(
                "Turn speed",
                format!("{:.0}°/s", def.turn_speed_degrees_per_second),
            ),
        ],
    });

    let caps = &def.work_capabilities;
    sections.push(UnitSkillsSection {
        title: "Work capability".into(),
        lines: vec![
            line(
                "Construction",
                if caps.can_construct {
                    format!("Capable ({:.2}× speed)", caps.construction_speed)
                } else {
                    "Not capable".into()
                },
            ),
            line(
                "Workstation operation",
                yes_no(caps.can_operate_workstation),
            ),
            line("Hauling", yes_no(caps.can_haul)),
        ],
    });

    let work_skill_lines = work_skill_catalog
        .enabled_definitions_ordered()
        .into_iter()
        .filter_map(|definition| {
            work_skill_value(world, work_skill_catalog, unit_id, &definition.id)
                .ok()
                .map(|value| line(definition.display_name.clone(), value.to_string()))
        })
        .collect();
    sections.push(UnitSkillsSection {
        title: "Work Skills".into(),
        lines: work_skill_lines,
    });

    let mut combat_lines = Vec::new();
    if let Some(weapon) = weapon_display_for_unit(record, unit_catalog, weapon_catalog) {
        append_weapon_hud_lines(&mut combat_lines, &weapon);
    } else {
        combat_lines.push("Weapon: —".into());
    }
    append_combat_state_lines(
        &mut combat_lines,
        record,
        combat_target_id(&record.combat_state),
    );
    sections.push(UnitSkillsSection {
        title: "Combat".into(),
        lines: combat_lines
            .into_iter()
            .filter_map(parse_colon_line)
            .collect(),
    });

    if let Some(profile) = NutritionProfile::from_definition(def) {
        let stage = evaluate_hunger_stage(record.nutrition.current, &profile);
        sections.push(UnitSkillsSection {
            title: "Nutrition".into(),
            lines: vec![
                line(
                    "Fullness",
                    format!("{:.0} / {:.0}", record.nutrition.current, profile.max),
                ),
                line("Hunger stage", hunger_stage_label(stage).to_string()),
            ],
        });
    }

    UnitSkillsPanelSnapshot {
        unit_id,
        title: def.display_name.clone(),
        sections,
    }
}

pub fn format_unit_skills_panel_text(snapshot: &UnitSkillsPanelSnapshot) -> String {
    let mut out = vec![snapshot.title.clone()];
    for section in &snapshot.sections {
        out.push(String::new());
        out.push(section.title.clone());
        for row in &section.lines {
            out.push(format!("  {}: {}", row.label, row.value));
        }
    }
    out.join("\n")
}

pub fn panel_contains_workforce_permission_controls(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("allowed farming")
        || lower.contains("allowed general labor")
        || lower.contains("workforce permission")
        || lower.contains("work permission")
}

fn line(label: impl Into<String>, value: impl Into<String>) -> UnitSkillsStatLine {
    UnitSkillsStatLine {
        label: label.into(),
        value: value.into(),
    }
}

fn yes_no(enabled: bool) -> String {
    if enabled {
        "Capable".into()
    } else {
        "Not capable".into()
    }
}

fn parse_colon_line(raw: String) -> Option<UnitSkillsStatLine> {
    let Some((label, value)) = raw.split_once(':') else {
        return Some(line(raw, ""));
    };
    Some(line(label.trim(), value.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkLayout, Heightfield, LocalPosition, UnitDefinitionId,
        UnitSource, UnitState, WorldPosition, create_unit, starter_weapon_definitions,
    };
    use bevy::prelude::Vec3;

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
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

    fn catalog() -> UnitCatalog {
        UnitCatalog::from_definitions(crate::world::starter_unit_definitions()).unwrap()
    }

    fn weapons() -> WeaponCatalog {
        WeaponCatalog::from_definitions(starter_weapon_definitions()).unwrap()
    }

    fn work_skills() -> crate::world::WorkSkillCatalog {
        crate::world::WorkSkillCatalog::default()
    }

    #[test]
    fn snapshot_uses_human_readable_names() {
        let catalog = catalog();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let snapshot =
            build_unit_skills_snapshot(unit_id, &world, &catalog, &weapons(), &work_skills())
                .unwrap();
        assert_eq!(snapshot.title, "Wolf");
        let text = format_unit_skills_panel_text(&snapshot);
        assert!(text.contains("Strength: 4"));
        assert!(!text.contains("U-"));
        assert!(!panel_contains_workforce_permission_controls(&text));
    }

    #[test]
    fn bandit_shows_work_capabilities_not_permissions() {
        let catalog = catalog();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let snapshot =
            build_unit_skills_snapshot(unit_id, &world, &catalog, &weapons(), &work_skills())
                .unwrap();
        let text = format_unit_skills_panel_text(&snapshot);
        assert!(text.contains("Work capability"));
        assert!(text.contains("Construction: Capable"));
        assert!(!panel_contains_workforce_permission_controls(&text));
    }

    #[test]
    fn snapshot_updates_when_hp_changes() {
        let catalog = catalog();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let first =
            build_unit_skills_snapshot(unit_id, &world, &catalog, &weapons(), &work_skills())
                .unwrap();
        world
            .mutate_unit(unit_id, |record| record.vitals.current_hp = 2)
            .expect("mutate");
        let second =
            build_unit_skills_snapshot(unit_id, &world, &catalog, &weapons(), &work_skills())
                .unwrap();
        assert_ne!(first, second);
        assert!(format_unit_skills_panel_text(&second).contains("HP: 2/5"));
    }

    #[test]
    fn removed_unit_returns_none() {
        let catalog = catalog();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        world.remove_unit_by_id(unit_id);
        assert!(
            build_unit_skills_snapshot(unit_id, &world, &catalog, &weapons(), &work_skills())
                .is_none()
        );
    }

    #[test]
    fn dead_unit_still_has_snapshot() {
        let catalog = catalog();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("wolf"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        world
            .set_unit_state(unit_id, UnitState::Dead)
            .expect("dead");
        let snapshot =
            build_unit_skills_snapshot(unit_id, &world, &catalog, &weapons(), &work_skills())
                .unwrap();
        assert!(format_unit_skills_panel_text(&snapshot).contains("State: Dead"));
    }

    #[test]
    fn work_skills_section_lists_catalog_display_names() {
        let catalog = catalog();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let text = format_unit_skills_panel_text(
            &build_unit_skills_snapshot(unit_id, &world, &catalog, &weapons(), &work_skills())
                .unwrap(),
        );
        assert!(text.contains("Work Skills"));
        assert!(text.contains("Farming: 0"));
        assert!(text.contains("General Labor: 0"));
        assert!(text.contains("Construction: 0"));
        assert!(text.contains("Cooking: 0"));
        assert!(text.contains("Science: 0"));
        assert!(text.contains("Smithing: 0"));
        assert!(!text.contains("farming:"));
    }

    #[test]
    fn work_skills_section_reflects_per_unit_values_and_selection() {
        let catalog = catalog();
        let work_skills = work_skills();
        let mut world = flat_world();
        let bob = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let larry = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(2.0, 2.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        crate::world::set_work_skill_value(
            &mut world,
            &work_skills,
            bob,
            &crate::world::WorkSkillId::new("farming"),
            10,
        )
        .unwrap();
        crate::world::set_work_skill_value(
            &mut world,
            &work_skills,
            larry,
            &crate::world::WorkSkillId::new("farming"),
            40,
        )
        .unwrap();
        let bob_text = format_unit_skills_panel_text(
            &build_unit_skills_snapshot(bob, &world, &catalog, &weapons(), &work_skills).unwrap(),
        );
        let larry_text = format_unit_skills_panel_text(
            &build_unit_skills_snapshot(larry, &world, &catalog, &weapons(), &work_skills).unwrap(),
        );
        assert!(bob_text.contains("Farming: 10"));
        assert!(larry_text.contains("Farming: 40"));
    }

    #[test]
    fn work_skills_section_includes_new_catalog_entries_without_ui_rewrite() {
        let catalog = catalog();
        let mut definitions = crate::world::starter_work_skill_definitions();
        definitions.push(crate::world::WorkSkillDefinition::new(
            "prospecting",
            "Prospecting",
            70,
        ));
        let work_skills = crate::world::WorkSkillCatalog::from_definitions(definitions).unwrap();
        let mut world = flat_world();
        let unit_id = create_unit(
            &catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(1.0, 1.0),
            UnitSource::Authored,
        )
        .unwrap()
        .id;
        let text = format_unit_skills_panel_text(
            &build_unit_skills_snapshot(unit_id, &world, &catalog, &weapons(), &work_skills)
                .unwrap(),
        );
        assert!(text.contains("Prospecting: 0"));
    }
}
