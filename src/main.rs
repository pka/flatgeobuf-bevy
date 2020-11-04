mod pan_orbit_camera;

use crate::pan_orbit_camera::PanOrbitCamera;
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
            pixel_size: Vec2::new(0.00003, 0.00003), // TODO: calculate from scale and center
        })
        // .spawn(Camera2dComponents::default())
        .add_plugin(pan_orbit_camera::PanOrbitCameraPlugin)
        .add_plugins(DefaultPlugins)
        .add_system(pan_map.system())
        .add_system(update_map.system())
        .run();
}

struct Map {
    center: Vec2,
    /// panning offset
    offset: Option<Vec3>,
    /// Size of center pixel in map coordinates
    pixel_size: Vec2,
}

struct PathDrawer {
    center: Vec2,
    pixel_size: Vec2,
    builder: PathBuilder,
}

impl GeomProcessor for PathDrawer {
    fn xy(&mut self, x: f64, y: f64, idx: usize) -> Result<()> {
        let x = (x as f32 - self.center.x()) / self.pixel_size.x();
        let y = (y as f32 - self.center.y()) / self.pixel_size.y();
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

fn pan_map(mousebtn: Res<Input<MouseButton>>, mut map: ResMut<Map>, query: Query<&PanOrbitCamera>) {
    // set map offset after end of panning
    if mousebtn.just_released(MouseButton::Left) {
        let mut focus = Vec3::default();
        for camera in query.iter() {
            focus = camera.focus;
        }
        map.offset = Some(focus);
    }
}

fn update_map(
    mut commands: Commands,
    window: Res<WindowDescriptor>,
    mut map: ResMut<Map>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if let Some(offset) = map.offset {
        map.offset = None;
        let start = Instant::now();
        let mut file = BufReader::new(File::open("osm-buildings-ch.fgb").unwrap());
        let mut fgb = FgbReader::open(&mut file).unwrap();
        let geometry_type = fgb.header().geometry_type();

        let wsize = Vec2::new(window.width as f32, window.height as f32);
        let center = Vec2::new(
            map.center.x() + offset.x() * map.pixel_size.x(),
            map.center.y() + offset.y() * map.pixel_size.y(),
        );
        let pixel_size = map.pixel_size;
        let bbox = (
            center.x() - wsize.x() / 2.0 * pixel_size.x(),
            center.y() - wsize.y() / 2.0 * pixel_size.y(),
            center.x() + wsize.x() / 2.0 * pixel_size.x(),
            center.y() + wsize.y() / 2.0 * pixel_size.y(),
        );

        let grey = materials.add(Color::rgb(0.25, 0.25, 0.25).into());
        let mut drawer = PathDrawer {
            center,
            pixel_size,
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

        // TODO: remove previous sprite
        commands.spawn(path.fill(grey, &mut meshes, offset, &FillOptions::default()));
        println!("Tesselate: {:?}", start.elapsed());
        // Calling `Path::stroke` or `Path::fill`, returns a `SpriteComponents`
        // bundle, which can be fed into Bevy's ECS system as `Entities`.
    }
}
