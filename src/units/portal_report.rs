//! Dev classification of the latest portal transition's presentation (IN-11c).
//!
//! [`crate::world::PortalTransitionTrace`] records the authoritative side of a
//! traversal; only the runtime layer can say whether the unit survived as a render
//! entity, where it ended up on screen, and whether ECS visibility hid it. This system
//! joins the two once per traversal — never per frame.

use bevy::prelude::*;

use crate::terrain::{TerrainRenderAssets, render_height_above_base};
use crate::world::{PortalTransitionEvent, UnitId, WorldConfig, WorldData};

use super::spawn::{UnitRenderIndex, unit_render_translation};

/// Distance from the destination floor plane that still counts as standing on it.
const FLOOR_PRESENTATION_TOLERANCE_UNITS: f32 = 0.75;

/// Why a transitioned unit is or is not on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalTransitionPresentation {
    /// Entity present, visible, and standing on the destination floor plane.
    Visible,
    /// The authoritative record survived but no render entity is indexed for it.
    RenderEntityMissing,
    /// The render entity exists but ECS visibility hides it.
    HiddenByVisibility,
    /// The render entity is placed far off the destination floor plane.
    OffFloorPlane,
    /// The render translation is not finite.
    NonFinitePosition,
    /// The unit record itself is gone.
    UnitRecordMissing,
}

impl PortalTransitionPresentation {
    pub fn summary(self) -> &'static str {
        match self {
            Self::Visible => "unit visible",
            Self::RenderEntityMissing => "render entity missing",
            Self::HiddenByVisibility => "unit hidden by visibility",
            Self::OffFloorPlane => "unit placed off the interior floor",
            Self::NonFinitePosition => "unit render position is not finite",
            Self::UnitRecordMissing => "unit record missing after transition",
        }
    }
}

/// Latest classified traversal, readable by dev panels.
#[derive(Resource, Debug, Clone, Default)]
pub struct LatestPortalTransitionReport {
    pub sequence: Option<u64>,
    pub unit_id: Option<UnitId>,
    pub presentation: Option<PortalTransitionPresentation>,
    /// Render-space distance from the destination floor plane.
    pub floor_offset_render_units: f32,
    /// `ViewVisibility` on the classification frame: framing, not ECS hiding.
    pub in_camera_view: Option<bool>,
    pub line: String,
}

/// Classify the presentation outcome of each newly recorded portal traversal.
pub fn report_portal_transition_presentation(
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    index: Res<UnitRenderIndex>,
    visibility: Query<(
        &Visibility,
        Option<&InheritedVisibility>,
        Option<&ViewVisibility>,
    )>,
    mut report: ResMut<LatestPortalTransitionReport>,
) {
    let Some(event) = world.portal_transition_trace().latest() else {
        return;
    };
    if report.sequence == Some(event.sequence) {
        return;
    }

    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let layout = config.chunk_layout();

    let Some(record) = world.get_unit(event.unit_id) else {
        *report = classified(event, PortalTransitionPresentation::UnitRecordMissing, 0.0);
        warn!("{}", report.line);
        return;
    };

    let render_translation = unit_render_translation(&world, record, layout, vertical_scale);
    let floor_render_y = event.destination_floor_y.map(|floor_y| {
        match crate::world::space_vertical_reference_y(
            &world,
            world.space_registry(),
            event.to_space,
        ) {
            Some(base) => render_height_above_base(base, floor_y, vertical_scale),
            None => crate::terrain::render_height(floor_y, vertical_scale),
        }
    });
    let floor_offset = floor_render_y
        .map(|floor| render_translation.y - floor)
        .unwrap_or(0.0);

    let entity = index.0.get(&event.unit_id).copied();
    // `ViewVisibility` is camera-dependent, so it describes framing rather than hiding and
    // is recorded as context instead of driving the classification.
    let mut in_camera_view = None;
    let presentation = if !render_translation.is_finite() {
        PortalTransitionPresentation::NonFinitePosition
    } else {
        match entity {
            None => PortalTransitionPresentation::RenderEntityMissing,
            Some(entity) => match visibility.get(entity) {
                Err(_) => PortalTransitionPresentation::RenderEntityMissing,
                Ok((own, inherited, view)) => {
                    in_camera_view = view.map(|value| value.get());
                    let hidden = matches!(own, Visibility::Hidden)
                        || inherited.is_some_and(|value| !value.get());
                    if hidden {
                        PortalTransitionPresentation::HiddenByVisibility
                    } else if floor_offset.abs() > FLOOR_PRESENTATION_TOLERANCE_UNITS {
                        PortalTransitionPresentation::OffFloorPlane
                    } else {
                        PortalTransitionPresentation::Visible
                    }
                }
            },
        }
    };

    *report = classified(event, presentation, floor_offset);
    report.in_camera_view = in_camera_view;
    if in_camera_view == Some(false) {
        report
            .line
            .push_str(" | outside the camera view this frame");
    }
    if matches!(presentation, PortalTransitionPresentation::Visible) {
        info!("{}", report.line);
    } else {
        warn!("{}", report.line);
    }
}

fn classified(
    event: &PortalTransitionEvent,
    presentation: PortalTransitionPresentation,
    floor_offset: f32,
) -> LatestPortalTransitionReport {
    let from = event.from_position;
    let to = event.grounded_position;
    let line = format!(
        "portal transition U-{:04} portal {}: space {} -> {} | {} | from chunk ({},{}) local {:?} \
         to chunk ({},{}) local {:?} | floor offset {:.2} render units | {} waypoints remaining",
        event.unit_id.raw(),
        event.portal_id.raw(),
        event.from_space.raw(),
        event.to_space.raw(),
        presentation.summary(),
        from.chunk.x,
        from.chunk.z,
        from.local.0,
        to.chunk.x,
        to.chunk.z,
        to.local.0,
        floor_offset,
        event.waypoints_remaining,
    );
    LatestPortalTransitionReport {
        sequence: Some(event.sequence),
        unit_id: Some(event.unit_id),
        presentation: Some(presentation),
        floor_offset_render_units: floor_offset,
        in_camera_view: None,
        line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingOwnership, ChunkCoord,
        ChunkData, ChunkId, DoodadCatalog, FootprintCatalog, Heightfield, LocalPosition,
        OccupancyCatalogs, PortalId, SpaceId, SpaceRecord, UnitCatalog, UnitDefinitionId,
        UnitSource, WorldPosition, create_unit, place_player_building,
    };
    use bevy::prelude::{App, MinimalPlugins, Quat, Update, Vec3};

    const FLOOR_OFFSET_METERS: f32 = 1.104;

    struct Fixture {
        app: App,
        unit_id: UnitId,
        entity: Entity,
        space_id: SpaceId,
    }

    fn fixture() -> Fixture {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<WorldConfig>();
        app.init_resource::<WorldData>();
        app.init_resource::<UnitRenderIndex>();
        app.init_resource::<LatestPortalTransitionReport>();
        app.add_systems(Update, report_portal_transition_presentation);

        let catalog = UnitCatalog::default();
        let building_catalog = BuildingCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let footprint = FootprintCatalog::default();
        let (unit_id, space_id) = {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            world.insert(
                ChunkId::new(ChunkCoord::new(0, 0)),
                ChunkData::new(
                    Heightfield::from_samples(3, 128.0, vec![0.0; 9]).expect("heightfield"),
                    Vec::new(),
                ),
            );
            let building_id = place_player_building(
                &building_catalog,
                &mut world,
                &BuildingDefinitionId::new("hut"),
                WorldPosition::new(ChunkCoord::new(0, 0), LocalPosition::new(Vec3::ZERO)),
                Quat::IDENTITY,
                BuildingOwnership::with_affiliation(Affiliation::Player),
                OccupancyCatalogs {
                    building: &building_catalog,
                    doodad: &doodad_catalog,
                    footprint: &footprint,
                },
            )
            .expect("place hut")
            .id;
            let registry = world.space_registry_mut();
            let space_id = registry.allocate_space_id();
            registry.insert_space(SpaceRecord {
                id: space_id,
                owning_building_id: Some(building_id),
                display_floor_label: "Ground".into(),
                visibility_group_id: 1,
                reference_elevation: FLOOR_OFFSET_METERS,
                floor_y_global: FLOOR_OFFSET_METERS,
                room_tag: None,
                enabled: true,
                walkable: true,
            });
            let unit_id = create_unit(
                &catalog,
                &mut world,
                &UnitDefinitionId::new("wolf"),
                WorldPosition::new(
                    ChunkCoord::new(0, 0),
                    LocalPosition::new(Vec3::new(20.0, FLOOR_OFFSET_METERS, 20.0)),
                ),
                UnitSource::Authored,
            )
            .expect("spawn unit")
            .id;
            world
                .set_unit_current_space(unit_id, space_id)
                .expect("enter space");
            (unit_id, space_id)
        };

        let entity = app
            .world_mut()
            .spawn((Visibility::Visible, InheritedVisibility::VISIBLE))
            .id();
        app.world_mut()
            .resource_mut::<UnitRenderIndex>()
            .0
            .insert(unit_id, entity);

        Fixture {
            app,
            unit_id,
            entity,
            space_id,
        }
    }

    fn record_entry(app: &mut App, unit_id: UnitId, space_id: SpaceId) {
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(20.0, FLOOR_OFFSET_METERS, 20.0)),
        );
        app.world_mut()
            .resource_mut::<WorldData>()
            .portal_transition_trace_mut()
            .record(PortalTransitionEvent {
                sequence: 0,
                unit_id,
                portal_id: PortalId::new(1),
                from_space: SpaceId::SURFACE,
                to_space: space_id,
                from_position: position,
                destination_position: position,
                grounded_position: position,
                destination_floor_y: Some(FLOOR_OFFSET_METERS),
                waypoints_remaining: 2,
            });
    }

    fn presentation(app: &App) -> PortalTransitionPresentation {
        app.world()
            .resource::<LatestPortalTransitionReport>()
            .presentation
            .expect("classified report")
    }

    #[test]
    fn present_and_visible_unit_is_reported_visible() {
        let mut fixture = fixture();
        record_entry(&mut fixture.app, fixture.unit_id, fixture.space_id);
        fixture.app.update();
        assert_eq!(
            presentation(&fixture.app),
            PortalTransitionPresentation::Visible
        );
        assert!(
            fixture
                .app
                .world()
                .resource::<LatestPortalTransitionReport>()
                .floor_offset_render_units
                .abs()
                < 0.01
        );
    }

    #[test]
    fn hidden_render_entity_is_distinguished_from_occlusion() {
        let mut fixture = fixture();
        *fixture
            .app
            .world_mut()
            .entity_mut(fixture.entity)
            .get_mut::<Visibility>()
            .expect("visibility") = Visibility::Hidden;
        record_entry(&mut fixture.app, fixture.unit_id, fixture.space_id);
        fixture.app.update();
        assert_eq!(
            presentation(&fixture.app),
            PortalTransitionPresentation::HiddenByVisibility
        );
    }

    #[test]
    fn missing_render_entity_is_reported() {
        let mut fixture = fixture();
        fixture
            .app
            .world_mut()
            .resource_mut::<UnitRenderIndex>()
            .0
            .remove(&fixture.unit_id);
        record_entry(&mut fixture.app, fixture.unit_id, fixture.space_id);
        fixture.app.update();
        assert_eq!(
            presentation(&fixture.app),
            PortalTransitionPresentation::RenderEntityMissing
        );
    }

    #[test]
    fn a_transition_is_classified_once() {
        let mut fixture = fixture();
        record_entry(&mut fixture.app, fixture.unit_id, fixture.space_id);
        fixture.app.update();
        let first = fixture
            .app
            .world()
            .resource::<LatestPortalTransitionReport>()
            .clone();
        fixture.app.update();
        let second = fixture
            .app
            .world()
            .resource::<LatestPortalTransitionReport>();
        assert_eq!(first.sequence, second.sequence);
        assert_eq!(first.line, second.line);
    }
}
