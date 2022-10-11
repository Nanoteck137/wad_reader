//! Module for utility functions

use crate::gltf;

pub fn triangulate(polygon: &[gltf::Vertex], clockwise: bool) -> Vec<u32> {
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

pub fn line_angle(a: &gltf::Vertex, b: &gltf::Vertex) -> f32 {
    (b.pos.z - a.pos.z).atan2(b.pos.x - a.pos.x)
}

pub fn point_on_line(
    a: &gltf::Vertex,
    b: &gltf::Vertex,
    c: &gltf::Vertex,
) -> bool {
    return (line_angle(a, b) - line_angle(b, c)).abs() < 0.05;
}

pub fn cleanup_lines(verts: &mut Vec<gltf::Vertex>) {
    for i in 0..verts.len() {
        let p1 = &verts[i % verts.len()];
        let p2 = &verts[i.wrapping_add(1) % verts.len()];
        let p3 = &verts[i.wrapping_add(2) % verts.len()];

        if point_on_line(p1, p2, p3) {
            verts.remove(i.wrapping_add(1) % verts.len());
        }
    }
}
