use bevy::prelude::*;
use bevy::render::{mesh, pipeline::PrimitiveTopology};
use flatgeobuf::*;
use geozero::error::Result;
use geozero::GeomProcessor;

#[derive(Default)]
struct Earcutr {
    center: (f64, f64),
    resolution: f64,
    // Earcutr input
    coords: Vec<f64>,
    hole_indices: Vec<usize>,
    // Bevy mesh data
    vertices: Vec<[f32; 2]>,
    triangles: Vec<u32>, // Max vertices: 4'294'967'295
    index_base: u32,
}

impl GeomProcessor for Earcutr {
    fn xy(&mut self, x: f64, y: f64, _idx: usize) -> Result<()> {
        // Convert to normalized device coordinates:
        // https://github.com/gfx-rs/gfx/tree/master/src/backend/dx12#normalized-coordinates
        let x = (x - self.center.0) / self.resolution;
        let y = (y - self.center.1) / self.resolution;
        self.coords.push(x);
        self.coords.push(y);
        Ok(())
    }
    fn linestring_begin(&mut self, tagged: bool, size: usize, idx: usize) -> Result<()> {
        if !tagged && idx > 0 {
            self.hole_indices.push(self.coords.len() / 2);
        }
        self.coords.reserve(size * 2);
        Ok(())
    }
    fn polygon_end(&mut self, _tagged: bool, _idx: usize) -> Result<()> {
        // Convert coords to mesh vertices
        self.vertices.reserve(self.coords.len() / 2);
        for coord in self.coords.chunks(2) {
            self.vertices.push([coord[0] as f32, coord[1] as f32]);
        }
        // Calculate and add triangles to mesh
        let triangles = earcutr::earcut(&self.coords, &self.hole_indices, 2);
        self.triangles.reserve(triangles.len());
        for idx in triangles {
            self.triangles.push(self.index_base + idx as u32);
        }
        self.index_base = self.vertices.len() as u32;

        // Reset polygon coords
        self.coords.clear();
        self.hole_indices.clear();

        Ok(())
    }
}

#[allow(dead_code)]
pub fn read_fgb(bbox: (f64, f64, f64, f64), center: Vec2, resolution: f32) -> Mesh {
    use seek_bufread::BufReader;
    use std::fs::File;

    let span = info_span!("read_fgb");
    let _read_fgb_span = span.enter();
    let mut file = BufReader::new(File::open("osm-buildings-zurich.fgb").unwrap());
    let mut fgb = FgbReader::open(&mut file).unwrap();

    let mut earcutr = Earcutr {
        center: (center.x as f64, center.y as f64),
        resolution: resolution.into(),
        ..Default::default()
    };

    let fcnt = fgb.select_bbox(bbox.0, bbox.1, bbox.2, bbox.3).unwrap();
    dbg!(fcnt);
    while let Some(feature) = fgb.next().unwrap() {
        feature.process_geom(&mut earcutr).unwrap();
    }

    earcutr.into()
}

#[allow(dead_code)]
pub async fn read_fgb_http(bbox: (f64, f64, f64, f64), center: Vec2, resolution: f32) -> Mesh {
    let span = info_span!("read_fgb_http");
    let _read_fgb_http_span = span.enter();
    let mut fgb = HttpFgbReader::open("https://pkg.sourcepole.ch/osm-buildings-zurich.fgb")
        .await
        .unwrap();

    let mut earcutr = Earcutr {
        center: (center.x as f64, center.y as f64),
        resolution: resolution.into(),
        ..Default::default()
    };

    fgb.select_bbox(bbox.0, bbox.1, bbox.2, bbox.3)
        .await
        .unwrap();
    while let Some(feature) = fgb.next().await.unwrap() {
        feature.process_geom(&mut earcutr).unwrap();
    }

    earcutr.into()
}

/// Converts a Earcutr struct into a bevy mesh.
impl From<Earcutr> for Mesh {
    fn from(data: Earcutr) -> Self {
        let num_vertices = data.vertices.len();
        let mut mesh = Self::new(PrimitiveTopology::TriangleList);
        mesh.set_indices(Some(mesh::Indices::U32(data.triangles)));
        mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, data.vertices);

        let mut normals = Vec::new();
        normals.resize(num_vertices, [0.0, 0.0, 0.0]);
        let mut uvs = Vec::new();
        uvs.resize(num_vertices, [0.0, 0.0]);

        mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.set_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

        mesh
    }
}
