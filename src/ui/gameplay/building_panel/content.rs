//! Building Panel content snapshot (BP2): bindings, production readout, inventory sections.

use crate::world::building_operational_efficiency;
use crate::world::{
    BuildingCatalog, BuildingId, BuildingInventoryBinding, BuildingOperationParams,
    FarmProductionPhase, InventoryId, InventoryProfileCatalog, OperationCatalog,
    OperationDefinitionId, OperationalLimitingFactor, PRODUCTION_PROGRESS_ONE_UNIT, WorldData,
    assess_production_execution, effective_inventory_binding_definitions, farm_growth_percent,
    farm_harvest_percent, format_efficiency_display, is_prispod_farm_definition,
};

/// Player-facing building panel snapshot derived from authoritative world data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingPanelSnapshot {
    pub header: BuildingPanelHeader,
    pub production: Option<BuildingPanelProduction>,
    pub inventories: Vec<BuildingPanelInventorySection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingPanelHeader {
    pub display_name: String,
    pub lifecycle_label: String,
    pub current_hp: u32,
    pub max_hp: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingPanelProduction {
    pub operation_name: String,
    pub progress_percent: Option<u32>,
    pub efficiency_display: Option<String>,
    pub blocking_label: Option<String>,
    pub enabled: bool,
    pub show_operation_selector: bool,
    pub operation_options: Vec<BuildingPanelOperationOption>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingPanelOperationOption {
    pub operation_id: OperationDefinitionId,
    pub display_name: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingPanelInventorySection {
    pub label: String,
    pub inventory_id: InventoryId,
    pub grid_width: u8,
    pub grid_height: u8,
    pub content_revision: u64,
}

impl BuildingPanelSnapshot {
    pub fn content_signature(&self) -> u64 {
        let mut sig = self.header.signature();
        if let Some(production) = &self.production {
            sig = sig.wrapping_mul(31).wrapping_add(production.signature());
        }
        for section in &self.inventories {
            sig = sig.wrapping_mul(31).wrapping_add(section.content_revision);
        }
        sig
    }
}

impl BuildingPanelHeader {
    fn signature(&self) -> u64 {
        (self.current_hp as u64)
            .wrapping_mul(1_000)
            .wrapping_add(self.max_hp as u64)
            .wrapping_add(self.lifecycle_label.len() as u64)
    }
}

impl BuildingPanelProduction {
    fn signature(&self) -> u64 {
        let mut sig = self.operation_name.len() as u64;
        sig = sig
            .wrapping_mul(31)
            .wrapping_add(u64::from(self.enabled))
            .wrapping_mul(31)
            .wrapping_add(u64::from(self.show_operation_selector));
        for option in &self.operation_options {
            sig = sig
                .wrapping_mul(31)
                .wrapping_add(option.display_name.len() as u64)
                .wrapping_mul(31)
                .wrapping_add(u64::from(option.selected));
        }
        if let Some(progress) = self.progress_percent {
            sig = sig.wrapping_mul(31).wrapping_add(progress as u64);
        }
        if let Some(eff) = &self.efficiency_display {
            sig = sig.wrapping_mul(31).wrapping_add(eff.len() as u64);
        }
        if let Some(blocked) = &self.blocking_label {
            sig = sig.wrapping_mul(31).wrapping_add(blocked.len() as u64);
        }
        sig
    }
}

/// Build the panel snapshot for an open owned building.
pub fn build_building_panel_snapshot(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    operation_params: &mut BuildingOperationParams<'_>,
    profile_catalog: &InventoryProfileCatalog,
    building_id: BuildingId,
) -> Option<BuildingPanelSnapshot> {
    let record = world.get_building(building_id)?;
    let definition = building_catalog.get(&record.definition_id)?;

    let display_name = definition.display_name.clone();
    let header = BuildingPanelHeader {
        display_name,
        lifecycle_label: record.lifecycle_state.label().to_string(),
        current_hp: record.vitals.current_hp,
        max_hp: record.vitals.max_hp,
    };

    let production = if definition.supported_operations.is_empty() {
        None
    } else {
        Some(build_production_readout(
            world,
            building_catalog,
            operation_catalog,
            operation_params,
            building_id,
            definition,
        ))
    };

    let binding_store = world.building_inventory_binding_store();
    let bindings = crate::world::building_inventory_bindings(binding_store, building_id);
    let inventories = bindings
        .iter()
        .map(|binding| {
            inventory_section_for_binding(world, definition, profile_catalog, binding, bindings)
        })
        .collect();

    Some(BuildingPanelSnapshot {
        header,
        production,
        inventories,
    })
}

fn build_production_readout(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    operation_params: &mut BuildingOperationParams<'_>,
    building_id: BuildingId,
    definition: &crate::world::BuildingDefinition,
) -> BuildingPanelProduction {
    let policy = world.building_production_store().get_policy(building_id);
    let enabled = policy.map(|policy| policy.enabled).unwrap_or(false);
    let explicit_selected = policy.and_then(|policy| policy.selected_operation.clone());
    let effective_operation_id = explicit_selected
        .clone()
        .or_else(|| definition.resolved_default_operation());
    let selected_operation = effective_operation_id
        .as_ref()
        .and_then(|id| operation_catalog.get(id));

    let operation_name = selected_operation
        .map(|op| op.display_name.clone())
        .unwrap_or_else(|| "No operation selected".to_string());

    let show_operation_selector = definition.supported_operations.len() > 1;
    let operation_options = if show_operation_selector {
        definition
            .supported_operations
            .iter()
            .filter_map(|operation_id| {
                operation_catalog
                    .get(operation_id)
                    .map(|operation| BuildingPanelOperationOption {
                        operation_id: operation_id.clone(),
                        display_name: operation.display_name.clone(),
                        selected: effective_operation_id.as_ref() == Some(operation_id),
                    })
            })
            .collect()
    } else {
        Vec::new()
    };

    let (operation_name, progress_percent) = if is_prispod_farm_definition(definition) {
        let store = world.building_production_store();
        let phase = store
            .farm_state(building_id)
            .map(|farm| farm.phase)
            .unwrap_or(FarmProductionPhase::Growing);
        match phase {
            FarmProductionPhase::Growing => (
                "Growing".to_string(),
                Some(farm_growth_percent(store, building_id)),
            ),
            FarmProductionPhase::ReadyToHarvest => ("Ready to Harvest".to_string(), None),
            FarmProductionPhase::Harvesting => (
                "Harvesting".to_string(),
                Some(farm_harvest_percent(store, building_id)),
            ),
        }
    } else {
        let progress_percent = world
            .building_production_store()
            .get_state(building_id)
            .map(|state| {
                ((state.progress.value() as u128 * 100) / PRODUCTION_PROGRESS_ONE_UNIT as u128)
                    as u32
            });
        (operation_name, progress_percent)
    };

    let operational_report = selected_operation.and_then(|op| {
        let mut ctx = operation_params.efficiency_context(world, building_catalog);
        building_operational_efficiency(&mut ctx, building_id, Some(op)).ok()
    });

    let efficiency_display = operational_report
        .as_ref()
        .map(|report| format_efficiency_display(report.final_output_efficiency_basis_points));

    let blocking_label = operational_report
        .filter(|report| report.limiting_factor != OperationalLimitingFactor::None)
        .map(|report| report.limiting_factor.label().to_string())
        .or_else(|| {
            selected_operation.and_then(|op| {
                assess_production_execution(
                    world,
                    operation_params.inventory_ctx,
                    building_id,
                    op,
                    definition,
                )
                .blocking
                .map(|failure| failure.limiting_factor().label().to_string())
            })
        });

    BuildingPanelProduction {
        operation_name,
        progress_percent,
        efficiency_display,
        blocking_label,
        enabled,
        show_operation_selector,
        operation_options,
    }
}

fn inventory_section_for_binding(
    world: &WorldData,
    definition: &crate::world::BuildingDefinition,
    profile_catalog: &InventoryProfileCatalog,
    binding: &BuildingInventoryBinding,
    all_bindings: &[BuildingInventoryBinding],
) -> BuildingPanelInventorySection {
    let (grid_width, grid_height, content_revision) =
        if let Some(record) = world.inventory_store().get(binding.inventory_id) {
            (
                record.grid_width(),
                record.grid_height(),
                inventory_content_revision(record),
            )
        } else {
            let authored = effective_inventory_binding_definitions(definition)
                .into_iter()
                .find(|authored| authored.binding_id == binding.binding_id);
            let (grid_width, grid_height) = authored
                .and_then(|authored| profile_catalog.get(&authored.profile_id))
                .map(|profile| (profile.grid_width, profile.grid_height))
                .unwrap_or((1, 1));
            (grid_width, grid_height, 0)
        };
    BuildingPanelInventorySection {
        label: binding_section_label(binding, all_bindings),
        inventory_id: binding.inventory_id,
        grid_width,
        grid_height,
        content_revision,
    }
}

fn inventory_content_revision(record: &crate::world::InventoryRecord) -> u64 {
    record.placed_entries().len() as u64 * 10_000 + record.total_mass_grams() as u64
}

/// Human-facing section label for one binding.
pub fn binding_section_label(
    binding: &BuildingInventoryBinding,
    all_bindings: &[BuildingInventoryBinding],
) -> String {
    if let Some(label) = binding.label.as_ref().filter(|label| !label.is_empty()) {
        return label.clone();
    }
    let role_label = binding.role.label();
    let same_role = all_bindings
        .iter()
        .filter(|other| other.role == binding.role)
        .count();
    if same_role <= 1 {
        return role_label.to_string();
    }
    humanize_binding_id(binding.binding_id.as_str())
}

fn humanize_binding_id(binding_id: &str) -> String {
    binding_id
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, BuildingCategoryCatalog, BuildingDefinition, BuildingId,
        BuildingInventoryBinding, BuildingInventoryRole, BuildingOwnership, BuildingSource,
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, InventoryCatalogCtx,
        InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, LocalPosition, WorldData,
        WorldPosition, create_building_with_inventory, place_stack_first_fit,
        starter_building_definitions, starter_inventory_profile_definitions,
        starter_item_category_definitions, starter_item_definitions, starter_operation_definitions,
    };
    use bevy::prelude::{Quat, Vec3};

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
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

    fn test_ctx() -> &'static InventoryCatalogCtx<'static> {
        static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
        CTX.get_or_init(|| {
            let categories =
                ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
            let items =
                ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
            let profiles =
                InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                    .unwrap();
            let items = Box::leak(Box::new(items));
            let categories = Box::leak(Box::new(categories));
            let profiles = Box::leak(Box::new(profiles));
            InventoryCatalogCtx::new(items, categories, profiles)
        })
    }

    fn definition(id: &str) -> BuildingDefinition {
        starter_building_definitions()
            .into_iter()
            .find(|def| def.id.as_str() == id)
            .unwrap_or_else(|| panic!("missing starter building {id}"))
    }

    fn place(
        world: &mut WorldData,
        building_catalog: &BuildingCatalog,
        definition: &BuildingDefinition,
    ) -> BuildingId {
        let record = create_building_with_inventory(
            building_catalog,
            world,
            &definition.id,
            pos(1.0, 1.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_ctx(),
        )
        .unwrap();
        record.id
    }

    fn building_catalog(definitions: Vec<BuildingDefinition>) -> BuildingCatalog {
        let categories = BuildingCategoryCatalog::default();
        BuildingCatalog::from_definitions(definitions, &categories).unwrap()
    }

    fn shared_operation_catalog() -> &'static OperationCatalog {
        static CATALOG: std::sync::OnceLock<OperationCatalog> = std::sync::OnceLock::new();
        CATALOG.get_or_init(|| {
            OperationCatalog::from_definitions(starter_operation_definitions()).unwrap()
        })
    }

    fn shared_inventory_profiles() -> &'static InventoryProfileCatalog {
        static CATALOG: std::sync::OnceLock<InventoryProfileCatalog> = std::sync::OnceLock::new();
        CATALOG.get_or_init(|| {
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap()
        })
    }

    struct TestOperationBundle {
        field_catalog: crate::world::TerrainFieldCatalog,
        requirement_catalog: crate::world::BuildingFieldRequirementCatalog,
        profile_catalog: crate::world::FieldResponseProfileCatalog,
        footprint_catalog: crate::world::FootprintCatalog,
        assessment_store: crate::world::BuildingTerrainAssessmentStore,
    }

    impl TestOperationBundle {
        fn new() -> Self {
            Self {
                field_catalog: crate::world::TerrainFieldCatalog::default(),
                requirement_catalog: crate::world::BuildingFieldRequirementCatalog::default(),
                profile_catalog: crate::world::FieldResponseProfileCatalog::default(),
                footprint_catalog: crate::world::FootprintCatalog::default(),
                assessment_store: crate::world::BuildingTerrainAssessmentStore::default(),
            }
        }

        fn params(&mut self) -> BuildingOperationParams<'_> {
            operation_params(
                shared_operation_catalog(),
                &mut self.assessment_store,
                &self.field_catalog,
                &self.requirement_catalog,
                &self.profile_catalog,
                &self.footprint_catalog,
            )
        }
    }

    fn operation_params<'a>(
        operation_catalog: &'a OperationCatalog,
        assessment_store: &'a mut crate::world::BuildingTerrainAssessmentStore,
        field_catalog: &'a crate::world::TerrainFieldCatalog,
        requirement_catalog: &'a crate::world::BuildingFieldRequirementCatalog,
        profile_catalog: &'a crate::world::FieldResponseProfileCatalog,
        footprint_catalog: &'a crate::world::FootprintCatalog,
    ) -> BuildingOperationParams<'a> {
        BuildingOperationParams {
            field_catalog,
            requirement_catalog,
            profile_catalog,
            footprint_catalog,
            operation_catalog,
            inventory_ctx: test_ctx(),
            requirement_revision: 0,
            profile_revision: 0,
            assessment_store,
        }
    }

    #[test]
    fn farm_uses_binding_store_not_legacy_inventory_id() {
        let farm = definition("prispod_farm");
        let catalog = building_catalog(vec![farm.clone()]);
        let mut world = flat_world();
        let building_id = place(&mut world, &catalog, &farm);
        let bindings = world
            .building_inventory_binding_store()
            .get(building_id)
            .expect("binding set");
        assert_eq!(bindings.len(), 1);
        let mut bundle = TestOperationBundle::new();
        let mut params = bundle.params();
        let snapshot = build_building_panel_snapshot(
            &world,
            &catalog,
            shared_operation_catalog(),
            &mut params,
            shared_inventory_profiles(),
            building_id,
        )
        .unwrap();
        assert_eq!(snapshot.inventories.len(), 1);
        assert_eq!(
            snapshot.inventories[0].inventory_id,
            bindings.bindings()[0].inventory_id
        );
        assert_eq!(snapshot.inventories[0].label, "Output");
    }

    #[test]
    fn farm_output_grid_is_one_by_one_without_input_sections() {
        let farm = definition("prispod_farm");
        let catalog = building_catalog(vec![farm.clone()]);
        let mut world = flat_world();
        let building_id = place(&mut world, &catalog, &farm);
        let mut bundle = TestOperationBundle::new();
        let mut params = bundle.params();
        let snapshot = build_building_panel_snapshot(
            &world,
            &catalog,
            shared_operation_catalog(),
            &mut params,
            shared_inventory_profiles(),
            building_id,
        )
        .unwrap();
        assert_eq!(snapshot.inventories.len(), 1);
        assert_eq!(snapshot.inventories[0].label, "Output");
        assert_eq!(snapshot.inventories[0].grid_width, 1);
        assert_eq!(snapshot.inventories[0].grid_height, 1);
        assert!(
            snapshot
                .inventories
                .iter()
                .all(|section| section.label != "Input"
                    && section.label != "Fuel"
                    && section.label != "Waste")
        );
    }

    #[test]
    fn smelter_exposes_all_binding_grids_with_dimensions() {
        let smelter = definition("smelter");
        let catalog = building_catalog(vec![smelter.clone()]);
        let mut world = flat_world();
        let building_id = place(&mut world, &catalog, &smelter);
        let mut bundle = TestOperationBundle::new();
        let mut params = bundle.params();
        let snapshot = build_building_panel_snapshot(
            &world,
            &catalog,
            shared_operation_catalog(),
            &mut params,
            shared_inventory_profiles(),
            building_id,
        )
        .unwrap();
        assert_eq!(snapshot.inventories.len(), 4);
        let labels: Vec<_> = snapshot
            .inventories
            .iter()
            .map(|s| s.label.as_str())
            .collect();
        assert!(labels.contains(&"Input"));
        assert!(labels.contains(&"Fuel"));
        assert!(labels.contains(&"Output"));
        assert!(labels.contains(&"Waste"));
        let input = snapshot
            .inventories
            .iter()
            .find(|s| s.label == "Input")
            .unwrap();
        assert_eq!((input.grid_width, input.grid_height), (8, 8));
        let fuel = snapshot
            .inventories
            .iter()
            .find(|s| s.label == "Fuel")
            .unwrap();
        assert_eq!((fuel.grid_width, fuel.grid_height), (4, 4));
    }

    #[test]
    fn production_building_shows_operation_display_name() {
        let farm = definition("prispod_farm");
        let catalog = building_catalog(vec![farm.clone()]);
        let mut world = flat_world();
        let building_id = place(&mut world, &catalog, &farm);
        world
            .building_production_store_mut()
            .farm_state_mut(building_id)
            .growth_progress = crate::world::ProductionProgress(420_000);
        let mut bundle = TestOperationBundle::new();
        let mut params = bundle.params();
        let snapshot = build_building_panel_snapshot(
            &world,
            &catalog,
            shared_operation_catalog(),
            &mut params,
            shared_inventory_profiles(),
            building_id,
        )
        .unwrap();
        let production = snapshot.production.expect("farm production");
        assert_eq!(production.operation_name, "Growing");
        assert_eq!(production.progress_percent, Some(42));
    }

    #[test]
    fn storage_chest_has_no_production_readout() {
        let chest = definition("storage_chest");
        let catalog = building_catalog(vec![chest.clone()]);
        let mut world = flat_world();
        let building_id = place(&mut world, &catalog, &chest);
        let mut bundle = TestOperationBundle::new();
        let mut params = bundle.params();
        let snapshot = build_building_panel_snapshot(
            &world,
            &catalog,
            shared_operation_catalog(),
            &mut params,
            shared_inventory_profiles(),
            building_id,
        )
        .unwrap();
        assert!(snapshot.production.is_none());
        assert!(!snapshot.inventories.is_empty());
    }

    #[test]
    fn inventory_revision_changes_when_contents_change() {
        let farm = definition("prispod_farm");
        let catalog = building_catalog(vec![farm.clone()]);
        let mut world = flat_world();
        let building_id = place(&mut world, &catalog, &farm);
        let mut bundle = TestOperationBundle::new();
        let mut params = bundle.params();
        let before = build_building_panel_snapshot(
            &world,
            &catalog,
            shared_operation_catalog(),
            &mut params,
            shared_inventory_profiles(),
            building_id,
        )
        .unwrap();
        let inventory_id = before.inventories[0].inventory_id;
        let (store, instances) = world.inventory_runtime_mut();
        place_stack_first_fit(
            store,
            instances,
            test_ctx(),
            inventory_id,
            crate::world::ItemDefinitionId::new("prispod"),
            1,
        )
        .unwrap();
        let mut params = bundle.params();
        let after = build_building_panel_snapshot(
            &world,
            &catalog,
            shared_operation_catalog(),
            &mut params,
            shared_inventory_profiles(),
            building_id,
        )
        .unwrap();
        assert_ne!(
            before.inventories[0].content_revision,
            after.inventories[0].content_revision
        );
        assert_ne!(before.content_signature(), after.content_signature());
    }

    #[test]
    fn binding_role_label_used_for_single_role() {
        let binding = BuildingInventoryBinding::new(
            "primary_output",
            BuildingInventoryRole::Output,
            InventoryId::new(1),
        );
        assert_eq!(
            binding_section_label(&binding, &[binding.clone()]),
            "Output"
        );
    }
}
