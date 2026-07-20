//! Production dependency graph derived from OperationCatalog (EP9).

use std::collections::{HashMap, HashSet};

use crate::world::ItemDefinitionId;
use crate::world::building::operation::OperationDefinitionId;
use crate::world::operation::{OperationCatalog, OperationOutputDefinition};

use super::types::{ProductionGraphEdge, ProductionPriorityCategory};

/// One recipe edge in the production graph (EP9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerRecipe {
    pub operation_id: OperationDefinitionId,
    pub output_item: ItemDefinitionId,
    pub output_quantity: u32,
    pub inputs: Vec<(ItemDefinitionId, u32)>,
    pub category: ProductionPriorityCategory,
}

/// Derived production graph — rebuilt each replan, never persisted (EP9).
#[derive(Debug, Clone, Default)]
pub struct ProductionGraph {
    pub producers_by_output: HashMap<ItemDefinitionId, Vec<ProducerRecipe>>,
    pub edges: Vec<ProductionGraphEdge>,
}

impl ProductionGraph {
    pub fn from_catalog(operation_catalog: &OperationCatalog) -> Self {
        let mut producers_by_output: HashMap<ItemDefinitionId, Vec<ProducerRecipe>> = HashMap::new();
        let mut edges = Vec::new();

        for definition in operation_catalog.enabled_definitions() {
            let inputs: Vec<(ItemDefinitionId, u32)> = definition
                .inputs
                .iter()
                .map(|input| (input.item_id.clone(), input.quantity))
                .collect();
            let category = map_operation_category(definition.category);

            for output in &definition.outputs {
                if let OperationOutputDefinition::Item {
                    item_id,
                    quantity,
                    destination_binding: _,
                } = output
                {
                    let recipe = ProducerRecipe {
                        operation_id: definition.id.clone(),
                        output_item: item_id.clone(),
                        output_quantity: *quantity,
                        inputs: inputs.clone(),
                        category,
                    };
                    producers_by_output
                        .entry(item_id.clone())
                        .or_default()
                        .push(recipe.clone());
                    for (input_item, _) in &inputs {
                        edges.push(ProductionGraphEdge {
                            output_item: item_id.clone(),
                            input_item: input_item.clone(),
                            operation_id: definition.id.clone(),
                        });
                    }
                }
            }
        }

        Self {
            producers_by_output,
            edges,
        }
    }

    pub fn producers_for(&self, item_id: &ItemDefinitionId) -> &[ProducerRecipe] {
        self.producers_by_output
            .get(item_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn select_producer(&self, item_id: &ItemDefinitionId) -> Option<&ProducerRecipe> {
        self.producers_for(item_id).iter().max_by_key(|recipe| {
            (
                recipe.category_priority(),
                recipe.output_quantity,
                recipe.operation_id.as_str(),
            )
        })
    }
}

impl ProducerRecipe {
    fn category_priority(&self) -> u8 {
        self.category.default_priority()
    }
}

fn map_operation_category(
    category: crate::world::operation::OperationCategory,
) -> ProductionPriorityCategory {
    use crate::world::operation::OperationCategory;
    match category {
        OperationCategory::Agriculture | OperationCategory::Crafting => {
            ProductionPriorityCategory::Food
        }
        OperationCategory::Processing => ProductionPriorityCategory::Construction,
        OperationCategory::Extraction => ProductionPriorityCategory::General,
        OperationCategory::Research => ProductionPriorityCategory::General,
        OperationCategory::Medical => ProductionPriorityCategory::Medicine,
        OperationCategory::Ritual => ProductionPriorityCategory::Luxury,
    }
}

/// Detect circular item dependencies in the production graph (EP9).
pub fn detect_production_cycles(graph: &ProductionGraph) -> Vec<Vec<ItemDefinitionId>> {
    let mut adjacency: HashMap<ItemDefinitionId, Vec<ItemDefinitionId>> = HashMap::new();
    for edge in &graph.edges {
        adjacency
            .entry(edge.output_item.clone())
            .or_default()
            .push(edge.input_item.clone());
    }

    let mut cycles = Vec::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    let mut stack = Vec::new();

    let nodes: Vec<ItemDefinitionId> = adjacency.keys().cloned().collect();
    for start in nodes {
        if visited.contains(&start) {
            continue;
        }
        dfs_cycles(
            &adjacency,
            &start,
            &mut visiting,
            &mut visited,
            &mut stack,
            &mut cycles,
        );
    }
    cycles
}

fn dfs_cycles(
    adjacency: &HashMap<ItemDefinitionId, Vec<ItemDefinitionId>>,
    node: &ItemDefinitionId,
    visiting: &mut HashSet<ItemDefinitionId>,
    visited: &mut HashSet<ItemDefinitionId>,
    stack: &mut Vec<ItemDefinitionId>,
    cycles: &mut Vec<Vec<ItemDefinitionId>>,
) {
    if visiting.contains(node) {
        if let Some(pos) = stack.iter().position(|item| item == node) {
            cycles.push(stack[pos..].to_vec());
        }
        return;
    }
    if visited.contains(node) {
        return;
    }
    visiting.insert(node.clone());
    stack.push(node.clone());
    if let Some(neighbors) = adjacency.get(node) {
        for neighbor in neighbors {
            dfs_cycles(adjacency, neighbor, visiting, visited, stack, cycles);
        }
    }
    stack.pop();
    visiting.remove(node);
    visited.insert(node.clone());
}

/// Propagate item demand through producer recipes (EP9).
pub fn propagate_demand(
    graph: &ProductionGraph,
    item_id: &ItemDefinitionId,
    quantity: u32,
    demand: &mut HashMap<ItemDefinitionId, u32>,
    depth: usize,
    max_depth: usize,
) -> Result<(), ItemDefinitionId> {
    if quantity == 0 {
        return Ok(());
    }
    if depth > max_depth {
        return Err(item_id.clone());
    }
    *demand.entry(item_id.clone()).or_default() += quantity;

    let Some(producer) = graph.select_producer(item_id) else {
        return Ok(());
    };
    let cycles_needed = quantity.div_ceil(producer.output_quantity.max(1));
    for (input_item, input_qty) in &producer.inputs {
        propagate_demand(
            graph,
            input_item,
            input_qty.saturating_mul(cycles_needed),
            demand,
            depth + 1,
            max_depth,
        )?;
    }
    Ok(())
}
