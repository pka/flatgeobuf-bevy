use bevy::prelude::*;
use bevy::render::{mesh, pipeline::PrimitiveTopology};
use flatgeobuf::*;
use geozero::error::Result;
use geozero::GeomProcessor;
use lyon::{
    math::{point, Point},
    path::Builder,
    tessellation::{BuffersBuilder, FillAttributes, FillOptions, FillTessellator, VertexBuffers},
};
use std::cell::RefCell;

struct PathDrawer {
    center: Vec2,
    resolution: f32,
    builder: RefCell<Builder>,
    // Bevy mesh
    vertices: Vec<[f32; 2]>,
    triangles: Vec<u32>, // Max vertices: 4'294'967'295
    index_base: u32,
}

impl GeomProcessor for PathDrawer {
    fn xy(&mut self, x: f64, y: f64, idx: usize) -> Result<()> {
        let x = (x as f32 - self.center.x()) / self.resolution;
        let y = (y as f32 - self.center.y()) / self.resolution;
        if idx == 0 {
            self.builder.borrow_mut().move_to(point(x, y));
        } else {
            self.builder.borrow_mut().line_to(point(x, y));
        }
        Ok(())
    }
    fn polygon_end(&mut self, _tagged: bool, _idx: usize) -> Result<()> {
        self.builder.borrow_mut().close();

        let builder = self.builder.replace(Builder::new());
        let path = builder.build();

        let mut tessellator = FillTessellator::new();
        let mut buffer = VertexBuffers::<[f32; 2], u32>::new();
        let options = FillOptions::default();
        tessellator
            .tessellate_path(
                path.as_slice(),
                &options,
                &mut BuffersBuilder::new(&mut buffer, |pos: Point, _: FillAttributes| {
                    [pos.x, pos.y]
                }),
            )
            .unwrap();

        // TODO: Use custom vertex buffer instead of copying vertices
        self.vertices.reserve(buffer.vertices.len() / 2);
        for i in 0..buffer.vertices.len() {
            self.vertices.push(buffer.vertices[i]);
        }

        self.triangles.reserve(buffer.indices.len());
        for idx in buffer.indices {
            self.triangles.push(self.index_base + idx as u32);
        }
        self.index_base = self.vertices.len() as u32;

        Ok(())
    }
}

#[allow(dead_code)]
pub fn read_fgb(bbox: (f64, f64, f64, f64), center: Vec2, resolution: f32) -> Mesh {
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
        builder: RefCell::new(Builder::new()),
        vertices: Vec::new(),
        triangles: Vec::new(),
        index_base: 0,
    };
    fgb.select_bbox(bbox.0, bbox.1, bbox.2, bbox.3).unwrap();
    while let Some(feature) = fgb.next().unwrap() {
        let geometry = feature.geometry().unwrap();
        geometry.process(&mut drawer, geometry_type).unwrap();
    }

    drawer.into()
}

#[allow(dead_code)]
pub async fn read_fgb_http(bbox: (f64, f64, f64, f64), center: Vec2, resolution: f32) -> Mesh {
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
        builder: RefCell::new(Builder::new()),
        vertices: Vec::new(),
        triangles: Vec::new(),
        index_base: 0,
    };
    while let Some(feature) = fgb.next().await.unwrap() {
        let geometry = feature.geometry().unwrap();
        geometry.process(&mut drawer, geometry_type).unwrap();
    }

    drawer.into()
}

/// Converts a PathDrawer struct into a bevy mesh.
impl From<PathDrawer> for Mesh {
    fn from(data: PathDrawer) -> Self {
        let num_vertices = data.vertices.len();
        let mut mesh = Self::new(PrimitiveTopology::TriangleList);
        mesh.set_indices(Some(mesh::Indices::U32(data.triangles)));
        mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, data.vertices);

        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        for _ in 0..num_vertices {
            normals.push([0.0, 0.0, 0.0]);
            uvs.push([0.0, 0.0]);
        }

        mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.set_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

        mesh
    }
}
