//! Module for utility functions

use std::path::Path;
use std::fs::File;
use std::io::{Read, Write, BufWriter};

use crate::polygon::Vertex;
use crate::texture::Texture;

pub fn array_to_string(arr: &[u8]) -> String {
    let null_pos = arr.iter().position(|&c| c == 0).unwrap_or(arr.len());
    let s = &arr[..null_pos];
    let s = std::str::from_utf8(&s).expect("Failed to convert array to str");

    s.to_string()
}

pub fn read_binary_file<P>(path: P) -> Vec<u8>
where
    P: AsRef<Path>,
{
    let mut file = File::open(path).unwrap();

    let mut result = Vec::new();
    file.read_to_end(&mut result).unwrap();

    result
}

pub fn write_binary_file<P>(path: P, data: &[u8])
where
    P: AsRef<Path>,
{
    let mut file = File::create(path).unwrap();
    file.write_all(data).unwrap();
}

pub fn write_texture_to_png(texture: &Texture) -> Vec<u8> {
    let mut result = Vec::new();
    {
        let ref mut file_writer = BufWriter::new(&mut result);

        let mut encoder = png::Encoder::new(
            file_writer,
            texture.width() as u32,
            texture.height() as u32,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&texture.pixels()).unwrap();
    }

    result
}

pub fn triangulate(polygon: &[Vertex], clockwise: bool) -> Vec<u32> {
    let mut indices = Vec::new();

    let p0 = 0u32;
    let mut p1 = 1u32;

    let mut index = 2;

    loop {
        if index >= polygon.len() {
            break;
        }

        let p2 = index as u32;

        if clockwise {
            indices.push(p0);
            indices.push(p1);
            indices.push(p2);
        } else {
            indices.push(p0);
            indices.push(p2);
            indices.push(p1);
        }

        p1 = p2;

        index += 1;
    }

    indices
}

pub fn line_angle(a: &Vertex, b: &Vertex) -> f32 {
    (b.pos.z - a.pos.z).atan2(b.pos.x - a.pos.x)
}

pub fn point_on_line(a: &Vertex, b: &Vertex, c: &Vertex) -> bool {
    return (line_angle(a, b) - line_angle(b, c)).abs() < 0.05;
}

pub fn cleanup_lines(verts: &mut Vec<Vertex>) {
    for i in 0..verts.len() {
        let p1 = &verts[i % verts.len()];
        let p2 = &verts[i.wrapping_add(1) % verts.len()];
        let p3 = &verts[i.wrapping_add(2) % verts.len()];

        if point_on_line(p1, p2, p3) {
            verts.remove(i.wrapping_add(1) % verts.len());
        }
    }
}
