use bevy::prelude::*;
use bevy_prototype_lyon::prelude::*;
use flatgeobuf::*;
use geozero::error::Result;
use geozero::GeomProcessor;

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

#[allow(dead_code)]
pub fn read_fgb(bbox: (f64, f64, f64, f64), center: Vec2, resolution: f32) -> Path {
    use std::fs::File;
    use std::io::BufReader;

    let span = info_span!("read_fgb");
    let _read_fgb_span = span.enter();
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
    path
}

#[allow(dead_code)]
pub async fn read_fgb_http(bbox: (f64, f64, f64, f64), center: Vec2, resolution: f32) -> Path {
    let span = info_span!("read_fgb_http");
    let _read_fgb_http_span = span.enter();
    let mut fgb = HttpFgbReader::open("https://pkg.sourcepole.ch/osm-buildings-zurich.fgb")
        .await
        .unwrap();
    let geometry_type = fgb.header().geometry_type();

    fgb.select_bbox(bbox.0, bbox.1, bbox.2, bbox.3)
        .await
        .unwrap();
    let mut drawer = PathDrawer {
        center,
        resolution,
        builder: PathBuilder::new(),
    };
    while let Some(feature) = fgb.next().await.unwrap() {
        let geometry = feature.geometry().unwrap();
        geometry.process(&mut drawer, geometry_type).unwrap();
    }

    drawer.builder.build()
}

pub fn tesselate(
    path: Path,
    offset: Vec3,
    material: Handle<ColorMaterial>,
    meshes: &mut ResMut<Assets<Mesh>>,
) -> SpriteBundle {
    let span = info_span!("tesselate");
    let _tesselate_span = span.enter();
    // let fill_options = FillOptions::default().with_intersections(false);
    let fill_options = FillOptions::default();
    path.fill(material, meshes, offset, &fill_options)
}
