//! Custom map format

use std::path::Path;
use std::fs::File;
use std::io::Write;

#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,

    pub color: [f32; 4],
}

impl Vertex {
    pub fn new(x: f32, y: f32, color: [f32; 4]) -> Self {
        Self {
            x, y, color
        }
    }
}

pub struct Map {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Map {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self {
            vertices,
            indices,
        }
    }

    pub fn save_to_file<P>(&self, filename: P) -> Option<()>
        where P: AsRef<Path>
    {
        let mut buffer = Vec::new();

        // Write out the header
        buffer.extend_from_slice(b"MIME");
        buffer.extend_from_slice(&1u32.to_le_bytes());

        // We can set a initial size here

        // Save the offset where we should write the vertex buffer header
        let vertex_buffer_header = buffer.len();
        buffer.extend_from_slice(&0u64.to_le_bytes());
        buffer.extend_from_slice(&0u64.to_le_bytes());

        // Save the offset where we should write the index buffer size
        let index_buffer_header = buffer.len();
        buffer.extend_from_slice(&0u64.to_le_bytes());
        buffer.extend_from_slice(&0u64.to_le_bytes());

        let vertex_buffer_offset = buffer.len();
        for vert in &self.vertices {
            // Vertex Position (x, y)
            buffer.extend_from_slice(&vert.x.to_le_bytes());
            buffer.extend_from_slice(&vert.y.to_le_bytes());

            // Vertex Color (r, g, b, a)
            buffer.extend_from_slice(&vert.color[0].to_le_bytes());
            buffer.extend_from_slice(&vert.color[1].to_le_bytes());
            buffer.extend_from_slice(&vert.color[2].to_le_bytes());
            buffer.extend_from_slice(&vert.color[3].to_le_bytes());
        }

        let index_buffer_offset = buffer.len();
        for index in &self.indices {
            buffer.extend_from_slice(&index.to_le_bytes());
        }

        // Write the vertex buffer offset
        let vertex_buffer_offset: u64 = vertex_buffer_offset.try_into().ok()?;
        let vertex_buffer_count: u64 = self.vertices.len().try_into().ok()?;
        buffer[vertex_buffer_header..vertex_buffer_header + 8].clone_from_slice(&vertex_buffer_offset.to_le_bytes());
        buffer[vertex_buffer_header + 8..vertex_buffer_header + 16].clone_from_slice(&vertex_buffer_count.to_le_bytes());

        let index_buffer_offset: u64 = index_buffer_offset.try_into().ok()?;
        let index_buffer_count: u64 = self.indices.len().try_into().ok()?;
        buffer[index_buffer_header..index_buffer_header + 8].clone_from_slice(&index_buffer_offset.to_le_bytes());
        buffer[index_buffer_header + 8..index_buffer_header + 16].clone_from_slice(&index_buffer_count.to_le_bytes());

        let mut file = File::create(filename).ok()?;
        file.write_all(&buffer[..]).ok()?;


        Some(())
    }
}
