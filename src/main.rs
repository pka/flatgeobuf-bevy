use bevy::{prelude::*, render::pass::ClearColor};
use bevy_prototype_lyon::prelude::*;
use flatgeobuf::*;
use geozero::error::Result;
use geozero::GeomProcessor;
use std::fs::File;
use std::io::BufReader;

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
        .add_default_plugins()
        .add_startup_system(setup.system())
        .run();
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

fn setup(
    mut commands: Commands,
    window: Res<WindowDescriptor>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mut file = BufReader::new(File::open("osm-buildings-ch.fgb").unwrap());
    let mut fgb = FgbReader::open(&mut file).unwrap();
    let geometry_type = fgb.header().geometry_type();

    let wsize = Vec2::new(window.width as f32, window.height as f32);
    let center = Vec2::new(8.53, 47.37);
    // Size of center pixel in map coordinates
    let pixel_size = Vec2::new(0.00003, 0.00003); // TODO: calculate from scale and center
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

    commands
        .spawn(Camera2dComponents::default())
        .spawn(path.fill(
            grey,
            &mut meshes,
            Vec3::new(0.0, 0.0, 0.0),
            &FillOptions::default(),
        ));
    // Calling `Path::stroke` or `Path::fill`, returns a `SpriteComponents`
    // bundle, which can be fed into Bevy's ECS system as `Entities`.
}
