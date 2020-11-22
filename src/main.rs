mod instant;
mod pan_orbit_camera;

use crate::instant::Instant;
use crate::pan_orbit_camera::{InputState, PanOrbitCamera};
#[cfg(target_arch = "wasm32")]
use bevy::tasks::IoTaskPool;
use bevy::{prelude::*, render::pass::ClearColor};
use bevy_prototype_lyon::prelude::*;
use flatgeobuf::*;
use geozero::error::Result;
use geozero::GeomProcessor;

fn main() {
    let mut app = App::build();
    app.add_event::<UpdateMapEvent>()
        .add_resource(ClearColor(Color::rgb(1.0, 1.0, 1.0)))
        .add_resource(WindowDescriptor {
            width: 978,
            height: 733,
            ..Default::default()
        })
        .add_resource(Map {
            center: Vec2::new(8.53, 47.37),
            offset: Vec3::default(),
            resolution: 0.00003,
            zoom: 1.0,
        })
        .add_plugin(pan_orbit_camera::PanOrbitCameraPlugin);

    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(DefaultPlugins)
        .add_system(update_map.system());

    #[cfg(target_arch = "wasm32")]
    app.add_plugins(bevy_webgl2::DefaultPlugins)
        .add_system(update_map_async.system());

    app.add_system(pan_or_zoom.system())
        .add_startup_system(setup_map.system())
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
        debug!(
            "map_events.send(UpdateMapEvent offset: {:?} zoom: {:?}",
            offset, zoom
        );
        map_events.send(UpdateMapEvent { offset, zoom });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn update_map(
    commands: &mut Commands,
    window: Res<WindowDescriptor>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut map: ResMut<Map>,
    mut map_event_reader: Local<EventReader<UpdateMapEvent>>,
    map_events: Res<Events<UpdateMapEvent>>,
) {
    if let Some(map_event) = map_event_reader.iter(&map_events).last() {
        let (center, resolution, bbox) = apply_map_event(&window, &mut map, map_event);
        let path = read_fgb(bbox, center, resolution);

        // Remove previous sprite
        if let Some(entity) = commands.current_entity() {
            commands.despawn(entity);
        }
        let grey = materials.add(Color::rgb(0.25, 0.25, 0.25).into());
        let start = Instant::now();
        commands.spawn(path.fill(grey, &mut meshes, map.offset, &FillOptions::default()));
        info!("Tesselate: {:?}", start.elapsed());
    }
}

#[cfg(target_arch = "wasm32")]
fn update_map_async(
    commands: &'static mut Commands,
    pool: Res<IoTaskPool>,
    window: Res<WindowDescriptor>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<'static, Assets<Mesh>>,
    mut map: ResMut<Map>,
    mut map_event_reader: Local<EventReader<UpdateMapEvent>>,
    map_events: Res<Events<UpdateMapEvent>>,
) {
    if let Some(map_event) = map_event_reader.iter(&map_events).last() {
        let (center, resolution, bbox) = apply_map_event(&window, &mut map, map_event);
        let offset = map.offset;
        let grey = materials.add(Color::rgb(0.25, 0.25, 0.25).into());
        pool.spawn(async move {
            let path = read_fgb_http(bbox, center, resolution).await;
            // Remove previous sprite
            if let Some(entity) = commands.current_entity() {
                commands.despawn(entity);
            }
            info!("commands.spawn path.fill");
            commands.spawn(path.fill(grey, &mut meshes, offset, &FillOptions::default()));
            info!("update_map finished");
        });
    }
}

fn apply_map_event(
    window: &Res<WindowDescriptor>,
    map: &mut ResMut<Map>,
    map_event: &UpdateMapEvent,
) -> (Vec2, f32, (f64, f64, f64, f64)) {
    if let Some(offset) = map_event.offset {
        map.offset = offset;
    }
    if let Some(zoom) = map_event.zoom {
        map.zoom = zoom;
    }
    let resolution = map.resolution * map.zoom;
    let center = Vec2::new(
        map.center.x() + map.offset.x() * resolution,
        map.center.y() + map.offset.y() * resolution,
    );
    let wsize = Vec2::new(window.width as f32, window.height as f32);
    let bbox = (
        (center.x() - wsize.x() / 2.0 * resolution) as f64,
        (center.y() - wsize.y() / 2.0 * resolution) as f64,
        (center.x() + wsize.x() / 2.0 * resolution) as f64,
        (center.y() + wsize.y() / 2.0 * resolution) as f64,
    );
    (center, resolution, bbox)
}

pub fn read_fgb(bbox: (f64, f64, f64, f64), center: Vec2, resolution: f32) -> Path {
    use std::fs::File;
    use std::io::BufReader;

    let start = Instant::now();
    let mut file = BufReader::new(File::open("osm-buildings-zurich.fgb").unwrap());
    let mut fgb = FgbReader::open(&mut file).unwrap();
    let geometry_type = fgb.header().geometry_type();

    let mut drawer = PathDrawer {
        center,
        resolution,
        builder: PathBuilder::new(),
    };
    fgb.select_bbox(bbox.0, bbox.1, bbox.2, bbox.3).unwrap();
    while let Some(feature) = fgb.next().unwrap() {
        let geometry = feature.geometry().unwrap();
        geometry.process(&mut drawer, geometry_type).unwrap();
    }
    let path = drawer.builder.build();
    info!("Read data into Lyon path: {:?}", start.elapsed());
    path
}

pub async fn read_fgb_http(bbox: (f64, f64, f64, f64), center: Vec2, resolution: f32) -> Path {
    info!("HttpFgbReader::open");
    let mut fgb = HttpFgbReader::open("https://pkg.sourcepole.ch/osm-buildings-zurich.fgb")
        .await
        .unwrap();
    let geometry_type = fgb.header().geometry_type();

    info!("select_bbox");
    fgb.select_bbox(bbox.0, bbox.1, bbox.2, bbox.3)
        .await
        .unwrap();
    let mut drawer = PathDrawer {
        center,
        resolution,
        builder: PathBuilder::new(),
    };
    info!("read polygons");
    while let Some(feature) = fgb.next().await.unwrap() {
        let geometry = feature.geometry().unwrap();
        geometry.process(&mut drawer, geometry_type).unwrap();
    }

    info!("drawer.builder");
    drawer.builder.build()
}
