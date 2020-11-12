mod pan_orbit_camera;

use crate::pan_orbit_camera::{InputState, PanOrbitCamera};
use bevy::{prelude::*, render::pass::ClearColor};
use bevy_prototype_lyon::prelude::*;
use flatgeobuf::*;
use geozero::error::Result;
use geozero::GeomProcessor;
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;

fn main() {
    App::build()
        .add_event::<UpdateMapEvent>()
        .add_resource(ClearColor(Color::rgb(1.0, 1.0, 1.0)))
        .add_resource(WindowDescriptor {
            title: "Bevy map".to_string(),
            width: 1067,
            height: 800,
            vsync: true,
            resizable: false,
            ..Default::default()
        })
        .add_resource(Map {
            center: Vec2::new(8.53, 47.37),
            offset: Vec3::default(),
            resolution: 0.00003,
            zoom: 1.0,
        })
        // .spawn(Camera2dComponents::default())
        .add_plugin(pan_orbit_camera::PanOrbitCameraPlugin)
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup_map.system())
        .add_system(pan_or_zoom.system())
        .add_system(update_map.system())
        .run();
}

struct Map {
    center: Vec2,
    /// panning offset
    offset: Vec3,
    /// Map units per pixel at center. (e.g. m/pixel or degree/pixel)
    resolution: f32,
    /// zoom factor
    zoom: f32,
}

struct PathDrawer {
    center: Vec2,
    resolution: f32,
    builder: PathBuilder,
}

impl GeomProcessor for PathDrawer {
    fn xy(&mut self, x: f64, y: f64, idx: usize) -> Result<()> {
        let x = (x as f32 - self.center.x()) / self.resolution;
        let y = (y as f32 - self.center.y()) / self.resolution;
        if idx == 0 {
            self.builder.move_to(point(x, y));
        } else {
            self.builder.line_to(point(x, y));
        }
        Ok(())
    }
    fn polygon_end(&mut self, _tagged: bool, _idx: usize) -> Result<()> {
        self.builder.close();
        Ok(())
    }
}

struct UpdateMapEvent {
    offset: Option<Vec3>,
    zoom: Option<f32>,
}

const PAN_DELAY: u128 = 200;
const ZOOM_DELAY: u128 = 150;

fn setup_map(mut map_events: ResMut<Events<UpdateMapEvent>>) {
    map_events.send(UpdateMapEvent {
        offset: Some(Vec3::default()),
        zoom: Some(1.0),
    });
}

fn pan_or_zoom(
    mut state: ResMut<InputState>,
    mousebtn: Res<Input<MouseButton>>,
    mut map_events: ResMut<Events<UpdateMapEvent>>,
    query: Query<(&PanOrbitCamera, &Transform)>,
) {
    let mut offset = None;
    let mut zoom = None;
    let motion_paused = state
        .last_motion
        .map(|last| last.elapsed().as_millis() > PAN_DELAY)
        .unwrap_or(false);
    // set map offset after end of panning
    if mousebtn.just_released(MouseButton::Left) || motion_paused {
        for (camera, _) in query.iter().take(1) {
            offset = Some(camera.focus);
        }
        state.last_motion = None;
    }

    let zoom_paused = state
        .last_zoom
        .map(|last| last.elapsed().as_millis() > ZOOM_DELAY)
        .unwrap_or(false);
    // set map resolution after end of zooming
    if zoom_paused {
        for (_, transform) in query.iter().take(1) {
            let zfact = 1000.0 / transform.translation.z();
            zoom = Some(1.0 + (1.0 - zfact) * 20.0);
        }
        state.last_zoom = None;
    }
    if offset.is_some() || zoom.is_some() {
        map_events.send(UpdateMapEvent { offset, zoom });
    }
}

fn update_map(
    mut commands: Commands,
    window: Res<WindowDescriptor>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut map: ResMut<Map>,
    mut map_event_reader: Local<EventReader<UpdateMapEvent>>,
    map_events: Res<Events<UpdateMapEvent>>,
) {
    for map_event in map_event_reader.iter(&map_events) {
        if let Some(offset) = map_event.offset {
            map.offset = offset;
        }
        if let Some(zoom) = map_event.zoom {
            map.zoom = zoom;
        }

        let start = Instant::now();
        let mut file = BufReader::new(File::open("osm-buildings-ch.fgb").unwrap());
        let mut fgb = FgbReader::open(&mut file).unwrap();
        let geometry_type = fgb.header().geometry_type();

        let wsize = Vec2::new(window.width as f32, window.height as f32);
        let resolution = map.resolution * map.zoom;
        let center = Vec2::new(
            map.center.x() + map.offset.x() * resolution,
            map.center.y() + map.offset.y() * resolution,
        );
        let bbox = (
            center.x() - wsize.x() / 2.0 * resolution,
            center.y() - wsize.y() / 2.0 * resolution,
            center.x() + wsize.x() / 2.0 * resolution,
            center.y() + wsize.y() / 2.0 * resolution,
        );

        let grey = materials.add(Color::rgb(0.25, 0.25, 0.25).into());
        let mut drawer = PathDrawer {
            center,
            resolution,
            builder: PathBuilder::new(),
        };
        fgb.select_bbox(bbox.0 as f64, bbox.1 as f64, bbox.2 as f64, bbox.3 as f64)
            .unwrap();
        while let Some(feature) = fgb.next().unwrap() {
            let geometry = feature.geometry().unwrap();
            geometry.process(&mut drawer, geometry_type).unwrap();
        }

        // Calling `PathBuilder::build` will return a `Path` ready to be used to create
        // Bevy entities.
        let path = drawer.builder.build();
        println!("Read data into Lyon path: {:?}", start.elapsed());
        let start = Instant::now();

        // Remove previous sprite
        if let Some(entity) = commands.current_entity() {
            commands.despawn(entity);
        }
        commands.spawn(path.fill(grey, &mut meshes, map.offset, &FillOptions::default()));
        println!("Tesselate: {:?}", start.elapsed());
        // Calling `Path::stroke` or `Path::fill`, returns a `SpriteComponents`
        // bundle, which can be fed into Bevy's ECS system as `Entities`.
    }
}
