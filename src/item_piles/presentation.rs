use bevy::prelude::*;

use crate::world::authoring_transform::QuantizedOrientation;
use crate::world::{ItemCategoryId, ItemDefinition, ItemDefinitionId};

/// Configurable presentation for world item piles (IA0).
#[derive(Debug, Clone, Resource, Reflect)]
pub struct ItemPilePresentationSettings {
    /// Radius of the generic fallback sphere in meters.
    pub fallback_sphere_radius: f32,
    /// Fallback tint for stackable commodity piles.
    pub fallback_stack_color: Color,
    /// Fallback tint for unique-instance piles.
    pub fallback_unique_color: Color,
    /// Vertical offset for dev-mode floating labels.
    pub dev_label_offset_y: f32,
}

impl Default for ItemPilePresentationSettings {
    fn default() -> Self {
        Self {
            fallback_sphere_radius: 1.0,
            fallback_stack_color: Color::srgba(0.95, 0.72, 0.28, 1.0),
            fallback_unique_color: Color::srgba(0.55, 0.85, 1.0, 1.0),
            dev_label_offset_y: 1.2,
        }
    }
}

/// Why a pile uses the generic fallback mesh instead of its authored GLB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum ItemPileFallbackReason {
    MissingDefinition,
    MissingRenderKey,
    SceneNotReady,
}

impl ItemPileFallbackReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::MissingDefinition => "missing definition",
            Self::MissingRenderKey => "missing render key",
            Self::SceneNotReady => "scene not ready",
        }
    }
}

/// Marker on fallback mesh presentation for a world pile.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct ItemPileFallbackMesh {
    pub reason: ItemPileFallbackReason,
}

/// Marker on authored GLB scene presentation for a world pile.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct ItemPileSceneRoot;

/// Cached fallback sphere meshes and materials.
#[derive(Resource, Default)]
pub struct ItemPileFallbackAssets {
    mesh: Option<Handle<Mesh>>,
    stack_material: Option<Handle<StandardMaterial>>,
    unique_material: Option<Handle<StandardMaterial>>,
    category_materials: std::collections::HashMap<u64, Handle<StandardMaterial>>,
}

impl ItemPileFallbackAssets {
    pub fn mesh(
        &mut self,
        meshes: &mut Assets<Mesh>,
        settings: &ItemPilePresentationSettings,
    ) -> Handle<Mesh> {
        self.mesh
            .get_or_insert_with(|| meshes.add(Sphere::new(settings.fallback_sphere_radius)))
            .clone()
    }

    pub fn material_for_definition(
        &mut self,
        materials: &mut Assets<StandardMaterial>,
        settings: &ItemPilePresentationSettings,
        definition: Option<&ItemDefinition>,
        unique: bool,
    ) -> Handle<StandardMaterial> {
        if unique {
            return self
                .unique_material
                .get_or_insert_with(|| {
                    materials.add(fallback_material(settings.fallback_unique_color))
                })
                .clone();
        }

        let category_key = definition
            .map(|def| category_color_key(&def.category_id))
            .unwrap_or(0);
        if category_key == 0 {
            return self
                .stack_material
                .get_or_insert_with(|| {
                    materials.add(fallback_material(settings.fallback_stack_color))
                })
                .clone();
        }

        self.category_materials
            .entry(category_key)
            .or_insert_with(|| {
                let color = category_fallback_color(
                    definition.map(|def| &def.category_id),
                    settings.fallback_stack_color,
                );
                materials.add(fallback_material(color))
            })
            .clone()
    }
}

fn fallback_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }
}

fn category_color_key(category_id: &ItemCategoryId) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    category_id.as_str().hash(&mut hasher);
    hasher.finish()
}

fn category_fallback_color(category_id: Option<&ItemCategoryId>, base: Color) -> Color {
    let Some(category_id) = category_id else {
        return base;
    };
    let hash = category_color_key(category_id);
    let s = base.to_srgba();
    let shift = ((hash % 5) as f32) * 0.04;
    Color::srgba(
        (s.red + shift).min(1.0),
        (s.green + shift * 0.5).min(1.0),
        (s.blue + shift * 0.25).min(1.0),
        0.95,
    )
}

/// Human-readable pile label for dev overlays (IA0).
pub fn format_pile_dev_label(
    display_name: &str,
    quantity: Option<u32>,
    known_item: bool,
) -> String {
    match quantity {
        Some(qty) if known_item => format!("{display_name} x{qty}"),
        Some(qty) => format!("Unknown Item x{qty}"),
        None if known_item => display_name.to_string(),
        None => "Unknown Item".to_string(),
    }
}

/// Model-local correction for authored GLB items (not stored on [`WorldItemPileRecord`]).
///
/// Weapon meshes are imported blade-up (+Y). World placement aligns +Y to the surface normal,
/// which leaves them standing on the hilt; a +90° pitch lays the long axis into the ground plane.
fn item_pile_model_local_correction(definition: Option<&ItemDefinition>) -> Quat {
    if !item_definition_lies_flat_on_ground(definition) {
        return Quat::IDENTITY;
    }
    QuantizedOrientation::from_degrees(0.0, 90.0, 0.0)
        .unwrap_or(QuantizedOrientation::IDENTITY)
        .to_quat()
}

fn item_definition_lies_flat_on_ground(definition: Option<&ItemDefinition>) -> bool {
    let Some(definition) = definition else {
        return false;
    };
    definition.render_key.0.is_some() && definition.category_id.as_str() == "weapon"
}

/// Compose authored pile rotation with per-definition model correction for scene GLBs.
pub fn item_pile_visual_rotation(authored: Quat, definition: Option<&ItemDefinition>) -> Quat {
    (authored * item_pile_model_local_correction(definition)).normalize()
}

/// Resolve display metadata for a pile from catalog + instance store.
pub fn pile_display_metadata(
    definition_id: Option<&ItemDefinitionId>,
    items: &crate::world::ItemCatalog,
    quantity: Option<u32>,
) -> (String, bool) {
    let Some(definition_id) = definition_id else {
        return (
            format_pile_dev_label("Unknown Item", quantity, false),
            false,
        );
    };
    let Some(definition) = items.get(definition_id) else {
        return (
            format_pile_dev_label(definition_id.as_str(), quantity, false),
            false,
        );
    };
    (
        format_pile_dev_label(&definition.display_name, quantity, true),
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_scene_correction_lays_model_flat() {
        let definition = ItemDefinition::new(
            ItemDefinitionId::new("iron_sword"),
            "Iron Sword",
            "",
            ItemCategoryId::new("weapon"),
            1,
            3,
            false,
            1,
            1000,
            1,
            true,
        )
        .with_render_key(crate::world::ItemRenderKey::reserved("iron_sword"));
        let authored = Quat::IDENTITY;
        let visual = item_pile_visual_rotation(authored, Some(&definition));
        let up = visual * Vec3::Y;
        assert!(up.y.abs() < 0.1, "weapon rest pose should lie in XZ plane");
        assert!(up.length_squared() > 0.9);
    }

    #[test]
    fn container_scene_correction_is_identity() {
        let definition = ItemDefinition::new(
            ItemDefinitionId::new("leather_backpack"),
            "Backpack",
            "",
            ItemCategoryId::new("container"),
            2,
            3,
            false,
            1,
            500,
            1,
            true,
        )
        .with_render_key(crate::world::ItemRenderKey::reserved("leather_backpack"));
        let authored = Quat::IDENTITY;
        let visual = item_pile_visual_rotation(authored, Some(&definition));
        assert!((visual.x - authored.x).abs() < 1e-4);
        assert!((visual.y - authored.y).abs() < 1e-4);
        assert!((visual.z - authored.z).abs() < 1e-4);
        assert!((visual.w - authored.w).abs() < 1e-4);
    }

    #[test]
    fn dev_label_formats_stack_and_unknown() {
        assert_eq!(
            format_pile_dev_label("Iron Ore", Some(37), true),
            "Iron Ore x37"
        );
        assert_eq!(
            format_pile_dev_label("mystery", Some(12), false),
            "Unknown Item x12"
        );
        assert_eq!(format_pile_dev_label("Sword", None, true), "Sword");
        assert_eq!(format_pile_dev_label("?", None, false), "Unknown Item");
    }
}
