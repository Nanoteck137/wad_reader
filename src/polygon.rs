use crate::util;
use crate::math::{Vec2, Vec3, Vec4};

#[derive(Copy, Clone, Default, Debug)]
pub struct Vertex {
    pub pos: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub color: Vec4,
}

impl Vertex {
    pub fn new(pos: Vec3, normal: Vec3, uv: Vec2, color: Vec4) -> Self {
        Self {
            pos,
            normal,
            uv,
            color,
        }
    }
}

pub struct Mesh {
    pub vertex_buffer: Vec<Vertex>,
    pub index_buffer: Vec<u32>,
    pub texture_id: Option<usize>,
}

impl Mesh {
    pub fn new() -> Self {
        Self {
            vertex_buffer: Vec::new(),
            index_buffer: Vec::new(),
            texture_id: None,
        }
    }

    pub fn add_vertices(&mut self, vertices: &[Vertex], clockwise: bool) {
        let triangles = util::triangulate(&vertices, clockwise);

        let index_offset = self.vertex_buffer.len();

        for v in vertices {
            self.vertex_buffer.push(*v);
        }

        for i in &triangles {
            self.index_buffer.push(i + index_offset as u32);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Quad {
    pub points: [Vertex; 4],
    pub texture_id: usize,
}

impl Quad {
    pub fn new() -> Self {
        Self {
            points: [Default::default(); 4],
            texture_id: 0,
        }
    }
}
