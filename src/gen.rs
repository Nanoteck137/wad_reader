use crate::wad;
use crate::util;
use crate::texture::{Texture, TextureLoader, TextureQueue};
use crate::polygon::{Mesh, Quad, Vertex};
use crate::math::{Vec2, Vec3, Vec4};

fn queue_texture(
    texture_queue: &mut TextureQueue,
    texture_name: [u8; 8],
) -> Option<usize> {
    let null_pos = texture_name
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(texture_name.len());
    let texture_name = &texture_name[..null_pos];
    let texture_name = std::str::from_utf8(&texture_name)
        .expect("Failed to convert floor texture name to str");
    let texture_name = String::from(texture_name);

    texture_queue.enqueue(texture_name)
}

pub fn gen_floor(
    wad_map: &wad::Map,
    wad_sector: &wad::Sector,
    texture_loader: &TextureLoader,
    texture_queue: &mut TextureQueue,
) -> Mesh {
    let mut mesh = Mesh::new();

    let texture_id =
        queue_texture(texture_queue, wad_sector.floor_texture_name);
    mesh.texture_id = texture_id;

    let name = texture_queue.get_name_from_id(texture_id.unwrap()).unwrap();
    let texture = texture_loader.load(name).unwrap();

    let w = 1.0 / texture.width() as f32;
    let h = 1.0 / texture.height() as f32;

    let dim = Vec2::new(w, -h);

    for sub_sector in &wad_sector.sub_sectors {
        let mut verts = Vec::new();

        for segment in 0..sub_sector.count {
            let segment = wad_map.segments[sub_sector.start + segment];
            let start = wad_map.vertex(segment.start_vertex);

            let pos = Vec3::new(start.x, wad_sector.floor_height, start.y);
            let uv = Vec2::new(start.x, start.y) * dim;
            let color = Vec4::new(1.0, 1.0, 1.0, 1.0);
            let normal = Vec3::new(0.0, 1.0, 0.0);
            verts.push(Vertex::new(pos, normal, uv, color));
        }

        util::cleanup_lines(&mut verts);
        mesh.add_vertices(&verts, true);
    }

    mesh
}

pub fn gen_ceiling(
    wad_map: &wad::Map,
    wad_sector: &wad::Sector,
    texture_loader: &TextureLoader,
    texture_queue: &mut TextureQueue,
) -> Mesh {
    let mut mesh = Mesh::new();

    let texture_id =
        queue_texture(texture_queue, wad_sector.ceiling_texture_name);
    mesh.texture_id = texture_id;

    let name = texture_queue.get_name_from_id(texture_id.unwrap()).unwrap();
    let texture = texture_loader.load(name).unwrap();

    let w = 1.0 / texture.width() as f32;
    let h = 1.0 / texture.height() as f32;

    let dim = Vec2::new(w, -h);

    for sub_sector in &wad_sector.sub_sectors {
        let mut verts = Vec::new();

        for segment in 0..sub_sector.count {
            let segment = wad_map.segments[sub_sector.start + segment];
            let start = wad_map.vertex(segment.start_vertex);

            let pos = Vec3::new(start.x, wad_sector.ceiling_height, start.y);
            let uv = Vec2::new(start.x, start.y) * dim;
            let color = Vec4::new(1.0, 1.0, 1.0, 1.0);
            let normal = Vec3::new(0.0, -1.0, 0.0);
            verts.push(Vertex::new(pos, normal, uv, color));
        }

        util::cleanup_lines(&mut verts);
        mesh.add_vertices(&verts, false);
    }

    mesh
}

fn update_uv(
    quad: &mut Quad,
    texture: &Texture,
    length: f32,
    x_offset: f32,
    y_offset: f32,
    top: f32,
    bottom: f32,
    lower_peg: bool,
) {
    let height = (top - bottom).round();

    let mut y1 = y_offset;
    let mut y2 = y_offset + height;

    let texture_size =
        Vec2::new(texture.width() as f32, texture.height() as f32);

    if lower_peg {
        y2 = y_offset + texture_size.y as f32;
        y1 = y2 - height;
    }

    quad.points[0].uv =
        Vec2::new(x_offset, y1 + (top - quad.points[0].pos.y)) / texture_size;
    quad.points[1].uv =
        Vec2::new(x_offset, y2 + (bottom - quad.points[1].pos.y))
            / texture_size;
    quad.points[2].uv =
        Vec2::new(x_offset + length, y2 + (bottom - quad.points[2].pos.y))
            / texture_size;
    quad.points[3].uv =
        Vec2::new(x_offset + length, y1 + (top - quad.points[3].pos.y))
            / texture_size;
}

fn create_normal_wall_quad(
    texture_loader: &TextureLoader,
    texture_queue: &mut TextureQueue,
    sector: &wad::Sector,
    linedef: &wad::Linedef,
    sidedef: &wad::Sidedef,
    start: wad::Vertex,
    end: wad::Vertex,
) -> Quad {
    let texture_id =
        queue_texture(texture_queue, sidedef.middle_texture_name).unwrap();
    let name = texture_queue.get_name_from_id(texture_id).unwrap();
    let texture = texture_loader.load(name).unwrap();

    let x1 = start.x;
    let y1 = start.y;
    let x2 = end.x;
    let y2 = end.y;

    let pos0 = Vec3::new(x1, sector.ceiling_height, y1);
    let pos1 = Vec3::new(x1, sector.floor_height, y1);
    let pos2 = Vec3::new(x2, sector.floor_height, y2);
    let pos3 = Vec3::new(x2, sector.ceiling_height, y2);

    let a = pos1;
    let b = pos3;
    let c = pos2;

    let normal = ((b - a).cross(c - a)).normalize();

    let color = Vec4::new(1.0, 1.0, 1.0, 1.0);

    let dx = end.x - start.x;
    let dy = end.y - start.y;

    let length = (dx * dx + dy * dy).sqrt();

    let uv = Vec2::new(0.0, 0.0);
    let mut quad = Quad::new();
    quad.texture_id = texture_id;
    quad.points[0] = Vertex::new(pos0, normal, uv, color);
    quad.points[1] = Vertex::new(pos1, normal, uv, color);
    quad.points[2] = Vertex::new(pos2, normal, uv, color);
    quad.points[3] = Vertex::new(pos3, normal, uv, color);

    let lower_peg = linedef.flags & wad::LINEDEF_FLAG_LOWER_TEXTURE_UNPEGGED
        == wad::LINEDEF_FLAG_LOWER_TEXTURE_UNPEGGED;
    update_uv(
        &mut quad,
        &texture,
        length,
        sidedef.x_offset as f32,
        sidedef.y_offset as f32,
        sector.ceiling_height,
        sector.floor_height,
        lower_peg,
    );

    quad
}

fn gen_diff_wall(
    texture_loader: &TextureLoader,
    texture_queue: &TextureQueue,
    texture_id: usize,
    linedef: &wad::Linedef,
    sidedef: &wad::Sidedef,
    front_sector: &wad::Sector,
    back_sector: &wad::Sector,
    start: wad::Vertex,
    end: wad::Vertex,
    front: f32,
    back: f32,
    lower_quad: bool,
) -> Quad {
    let name = texture_queue.get_name_from_id(texture_id).unwrap();
    let texture = texture_loader.load(name).unwrap();

    let x1 = start.x;
    let y1 = start.y;
    let x2 = end.x;
    let y2 = end.y;

    let pos0 = Vec3::new(x1, back, y1);
    let pos1 = Vec3::new(x1, front, y1);
    let pos2 = Vec3::new(x2, front, y2);
    let pos3 = Vec3::new(x2, back, y2);

    let (a, b, c) = if lower_quad {
        (pos1, pos2, pos3)
    } else {
        (pos1, pos3, pos2)
    };

    // TODO(patrik): Check the normal
    let normal = ((b - a).cross(c - a)).normalize();

    let color = Vec4::new(1.0, 1.0, 1.0, 1.0);

    let dx = end.x - start.x;
    let dy = end.y - start.y;

    let length = (dx * dx + dy * dy).sqrt();

    let uv = Vec2::new(0.0, 0.0);
    let mut quad = Quad::new();
    quad.texture_id = texture_id;
    quad.points[0] = Vertex::new(pos0, normal, uv, color);
    quad.points[1] = Vertex::new(pos1, normal, uv, color);
    quad.points[2] = Vertex::new(pos2, normal, uv, color);
    quad.points[3] = Vertex::new(pos3, normal, uv, color);

    if lower_quad {
        let x_offset = sidedef.x_offset as f32;
        let mut y_offset = sidedef.y_offset as f32;
        if linedef.flags & wad::LINEDEF_FLAG_LOWER_TEXTURE_UNPEGGED
            == wad::LINEDEF_FLAG_LOWER_TEXTURE_UNPEGGED
        {
            y_offset += front_sector.ceiling_height - back_sector.floor_height;
        }

        update_uv(
            &mut quad, &texture, length, x_offset, y_offset, back, front,
            false,
        );
    } else {
        let x_offset = sidedef.x_offset as f32;
        let y_offset = sidedef.y_offset as f32;

        let upper_peg = linedef.flags
            & wad::LINEDEF_FLAG_UPPER_TEXTURE_UNPEGGED
            == wad::LINEDEF_FLAG_UPPER_TEXTURE_UNPEGGED;
        update_uv(
            &mut quad, &texture, length, x_offset, y_offset, back, front,
            !upper_peg,
        );
    }

    quad
}

fn gen_slope(
    start: wad::Vertex,
    end: wad::Vertex,
    front: f32,
    back: f32,
    diff: f32,
) -> Quad {
    let x1 = start.x;
    let y1 = start.y;
    let x2 = end.x;
    let y2 = end.y;

    let pos0 = Vec3::new(x1, back, y1);
    let pos1 = Vec3::new(x1, front, y1);
    let pos2 = Vec3::new(x2, front, y2);
    let pos3 = Vec3::new(x2, back, y2);

    let a = pos1;
    let b = pos3;
    let c = pos2;

    let normal = ((b - a).cross(c - a)).normalize();
    let x = (normal.x * 0.5) + 0.5;
    let y = (normal.y * 0.5) + 0.5;
    let z = (normal.z * 0.5) + 0.5;
    let color = Vec4::new(x, y, z, 1.0);
    //let color = Vec4::new(1.0, 1.0, 1.0, 1.0);
    let uv = Vec2::new(0.0, 0.0);

    let mut quad = Quad::new();
    quad.points[0] = Vertex::new(pos0, normal, uv, color);
    quad.points[1] = Vertex::new(pos1, normal, uv, color);
    quad.points[2] = Vertex::new(pos2, normal, uv, color);
    quad.points[3] = Vertex::new(pos3, normal, uv, color);

    if front < back {
        quad.points[1].pos += normal * diff;
        quad.points[2].pos += normal * diff;
    } else {
        quad.points[0].pos += normal * diff;
        quad.points[3].pos += normal * diff;
    }

    quad
}

pub fn gen_walls(
    wad_map: &wad::Map,
    wad_sector: &wad::Sector,
    texture_loader: &TextureLoader,
    texture_queue: &mut TextureQueue,
) -> (Vec<Quad>, Vec<Quad>) {
    let mut quads = Vec::new();
    let mut slope_quads = Vec::new();

    for sub_sector in &wad_sector.sub_sectors {
        for segment in 0..sub_sector.count {
            let segment = wad_map.segments[sub_sector.start + segment];
            if segment.linedef == 0xffff {
                continue;
            }

            let linedef = wad_map.linedefs[segment.linedef];
            let line = linedef.line;
            let start = wad_map.vertex(line.start_vertex);
            let end = wad_map.vertex(line.end_vertex);

            if linedef.flags & wad::LINEDEF_FLAG_TWO_SIDED
                != wad::LINEDEF_FLAG_TWO_SIDED
            {
                if let Some(sidedef) = linedef.front_sidedef {
                    let sidedef = wad_map.sidedefs[sidedef];

                    let quad = create_normal_wall_quad(
                        texture_loader,
                        texture_queue,
                        wad_sector,
                        &linedef,
                        &sidedef,
                        start,
                        end,
                    );

                    quads.push(quad);
                }
            }

            if linedef.front_sidedef.is_some()
                && linedef.back_sidedef.is_some()
            {
                let front_sidedef = linedef.front_sidedef.unwrap();
                let front_sidedef = wad_map.sidedefs[front_sidedef];

                let back_sidedef = linedef.back_sidedef.unwrap();
                let back_sidedef = wad_map.sidedefs[back_sidedef];

                let front_sector = &wad_map.sectors[front_sidedef.sector];
                let back_sector = &wad_map.sectors[back_sidedef.sector];

                // Generate the floor difference
                if front_sector.floor_height != back_sector.floor_height {
                    let front = front_sector.floor_height;
                    let back = back_sector.floor_height;

                    let min = front.min(back);
                    let max = front.max(back);

                    let diff = max - min;

                    if diff <= 24.0 {
                        let quad = gen_slope(start, end, front, back, diff);
                        slope_quads.push(quad);
                    }

                    let texture_id = queue_texture(
                        texture_queue,
                        front_sidedef.lower_texture_name,
                    )
                    .unwrap_or(0);

                    let quad = gen_diff_wall(
                        texture_loader,
                        texture_queue,
                        texture_id,
                        &linedef,
                        &front_sidedef,
                        front_sector,
                        back_sector,
                        start,
                        end,
                        front,
                        back,
                        true,
                    );

                    quads.push(quad);
                }

                // Generate the height difference
                if front_sector.ceiling_height != back_sector.ceiling_height {
                    let front = front_sector.ceiling_height;
                    let back = back_sector.ceiling_height;

                    let texture_id = queue_texture(
                        texture_queue,
                        front_sidedef.upper_texture_name,
                    )
                    .unwrap_or(0);

                    let quad = gen_diff_wall(
                        texture_loader,
                        texture_queue,
                        texture_id,
                        &linedef,
                        &front_sidedef,
                        front_sector,
                        back_sector,
                        start,
                        end,
                        back,
                        front,
                        false,
                    );

                    quads.push(quad);
                }
            }
        }
    }

    (quads, slope_quads)
}
