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
            offset: Some(Vec3::default()),
            resolution: 0.00003,
            zoom: Some(1.0),
        })
        // .spawn(Camera2dComponents::default())
        .add_plugin(pan_orbit_camera::PanOrbitCameraPlugin)
        .add_plugins(DefaultPlugins)
        .add_system(pan_or_zoom.system())
        .add_system(update_map.system())
        .run();
}

struct Map {
    center: Vec2,
    /// panning offset
    offset: Option<Vec3>,
    /// Map units per pixel at center. (e.g. m/pixel or degree/pixel)
    resolution: f32,
    /// zoom factor
    zoom: Option<f32>,
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
    fn linestring_end(&mut self, _tagged: bool, _idx: usize) -> Result<()> {
        self.builder.close();
        Ok(())
    }
}

const PAN_DELAY: u128 = 200;
const ZOOM_DELAY: u128 = 150;

fn pan_or_zoom(
    mut state: ResMut<InputState>,
    mousebtn: Res<Input<MouseButton>>,
    mut map: ResMut<Map>,
    query: Query<(&PanOrbitCamera, &Transform)>,
) {
    let motion_paused = state
        .last_motion
        .map(|last| last.elapsed().as_millis() > PAN_DELAY)
        .unwrap_or(false);
    // set map offset after end of panning
    if mousebtn.just_released(MouseButton::Left) || motion_paused {
        for (camera, _) in query.iter().take(1) {
            map.offset = Some(camera.focus);
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
            let z = transform.translation.z();
            let fact = 1000.0 / z;
            map.zoom = Some(fact * 2.0);
        }
        state.last_zoom = None;
    }
}

fn update_map(
    mut commands: Commands,
    window: Res<WindowDescriptor>,
    mut map: ResMut<Map>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if map.offset.is_some() || map.zoom.is_some() {
        let offset = map.offset.unwrap_or(Vec3::default());
        map.offset = None;
        let zoom = map.zoom.unwrap_or(1.0);
        map.zoom = None;

        let start = Instant::now();
        let mut file = BufReader::new(File::open("osm-buildings-ch.fgb").unwrap());
        let mut fgb = FgbReader::open(&mut file).unwrap();
        let geometry_type = fgb.header().geometry_type();

        let wsize = Vec2::new(window.width as f32, window.height as f32);
        let resolution = map.resolution * zoom;
        let center = Vec2::new(
            map.center.x() + offset.x() * resolution,
            map.center.y() + offset.y() * resolution,
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
        commands.spawn(path.fill(grey, &mut meshes, offset, &FillOptions::default()));
        println!("Tesselate: {:?}", start.elapsed());
        // Calling `Path::stroke` or `Path::fill`, returns a `SpriteComponents`
        // bundle, which can be fed into Bevy's ECS system as `Entities`.
    }
}
