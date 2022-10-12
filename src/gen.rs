use std::collections::HashSet;
use crate::wad;
use crate::util;
use crate::texture::{Texture, TextureLoader};
use crate::polygon::{Mesh, Quad, Vertex};
use crate::math::{Vec2, Vec3, Vec4};

pub struct Context {
    pub texture_loader: TextureLoader,
    pub texture_queue: HashSet<usize>,
}

impl Context {
    pub fn new(texture_loader: TextureLoader) -> Self {
        Self {
            texture_loader,
            texture_queue: HashSet::new(),
        }
    }

    fn texture(
        &mut self,
        texture_name: &str,
        queue_texture: bool,
    ) -> (usize, &Texture) {
        if let Some(texture) = self.texture_loader.load_from_name(texture_name)
        {
            if queue_texture {
                self.texture_queue.insert(texture.0);
            }
            return texture;
        } else {
            let texture = self.texture_loader.missing_texture();
            if queue_texture {
                self.texture_queue.insert(texture.0);
            }
            return texture;
        }
    }

    // fn queue_texture(&mut self, texture_name: &str) -> Option<usize> {
    //     // self.texture_queue.enqueue(texture_name)
    //     None
    // }
    //
    // fn get_texture_from_id(&self, texture_id: usize) -> Option<&Texture> {
    //     // TODO(patrik): Change this
    //     let name = self.texture_queue.get_name_from_id(texture_id).unwrap();
    //     self.texture_loader.load(name)
    // }
}

pub fn gen_floor(
    context: &mut Context,
    wad_map: &wad::Map,
    wad_sector: &wad::Sector,
) -> Mesh {
    let mut mesh = Mesh::new();

    let (texture_id, texture) =
        context.texture(&wad_sector.floor_texture, true);
    mesh.texture_id = Some(texture_id);

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
    context: &mut Context,
    wad_map: &wad::Map,
    wad_sector: &wad::Sector,
) -> Mesh {
    let mut mesh = Mesh::new();

    let (texture_id, texture) =
        context.texture(&wad_sector.ceiling_texture, true);
    mesh.texture_id = Some(texture_id);

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

fn create_quad(p1: Vec2, p2: Vec2, bottom: f32, top: f32) -> Quad {
    let pos0 = Vec3::new(p1.x, top, p1.y);
    let pos1 = Vec3::new(p1.x, bottom, p1.y);
    let pos2 = Vec3::new(p2.x, bottom, p2.y);
    let pos3 = Vec3::new(p2.x, top, p2.y);

    let a = pos1;
    let b = pos3;
    let c = pos2;

    // TODO(patrik): Check the normal
    let normal = ((b - a).cross(c - a)).normalize();

    // let x = (normal.x * 0.5) + 0.5;
    // let y = (normal.y * 0.5) + 0.5;
    // let z = (normal.z * 0.5) + 0.5;
    // let color = Vec4::new(x, y, z, 1.0);

    let color = Vec4::new(1.0, 1.0, 1.0, 1.0);
    let uv = Vec2::new(0.0, 0.0);

    let mut quad = Quad::new();
    quad.points[0] = Vertex::new(pos0, normal, uv, color);
    quad.points[1] = Vertex::new(pos1, normal, uv, color);
    quad.points[2] = Vertex::new(pos2, normal, uv, color);
    quad.points[3] = Vertex::new(pos3, normal, uv, color);

    quad
}

fn update_quad_uvs(
    quad: &mut Quad,
    texture: &Texture,
    length: f32,
    offset: Vec2,
    bottom: f32,
    top: f32,
    lower_peg: bool,
) {
    let height = (top - bottom).round();

    let mut y1 = offset.y;
    let mut y2 = offset.y + height;

    let texture_size =
        Vec2::new(texture.width() as f32, texture.height() as f32);

    if lower_peg {
        y2 = offset.y + texture_size.y as f32;
        y1 = y2 - height;
    }

    quad.points[0].uv =
        Vec2::new(offset.x, y1 + (top - quad.points[0].pos.y)) / texture_size;
    quad.points[1].uv =
        Vec2::new(offset.x, y2 + (bottom - quad.points[1].pos.y))
            / texture_size;
    quad.points[2].uv =
        Vec2::new(offset.x + length, y2 + (bottom - quad.points[2].pos.y))
            / texture_size;
    quad.points[3].uv =
        Vec2::new(offset.x + length, y1 + (top - quad.points[3].pos.y))
            / texture_size;
}

fn create_normal_wall_quad(
    context: &mut Context,
    sector: &wad::Sector,
    linedef: &wad::Linedef,
    sidedef: &wad::Sidedef,
    start: wad::Vertex,
    end: wad::Vertex,
) -> Quad {
    let (texture_id, texture) = context.texture(&sidedef.middle_texture, true);

    let start = Vec2::new(start.x, start.y);
    let end = Vec2::new(end.x, end.y);
    let mut quad =
        create_quad(start, end, sector.floor_height, sector.ceiling_height);
    quad.texture_id = texture_id;

    let length = (end - start).length();
    let offset = Vec2::new(sidedef.x_offset as f32, sidedef.y_offset as f32);

    let lower_peg = linedef
        .flags
        .contains(wad::LinedefFlags::LOWER_TEXTURE_UNPEGGED);
    update_quad_uvs(
        &mut quad,
        &texture,
        length,
        offset,
        sector.floor_height,
        sector.ceiling_height,
        lower_peg,
    );

    quad
}

fn gen_diff_wall(
    texture: &Texture,
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
    let start = Vec2::new(start.x, start.y);
    let end = Vec2::new(end.x, end.y);
    let mut quad = create_quad(start, end, front, back);

    let length = (end - start).length();

    if lower_quad {
        let mut offset =
            Vec2::new(sidedef.x_offset as f32, sidedef.y_offset as f32);
        if linedef
            .flags
            .contains(wad::LinedefFlags::LOWER_TEXTURE_UNPEGGED)
        {
            offset.y += front_sector.ceiling_height - back_sector.floor_height;
        }

        update_quad_uvs(
            &mut quad, &texture, length, offset, front, back, false,
        );
    } else {
        let offset =
            Vec2::new(sidedef.x_offset as f32, sidedef.y_offset as f32);

        let upper_peg = linedef
            .flags
            .contains(wad::LinedefFlags::UPPER_TEXTURE_UNPEGGED);
        update_quad_uvs(
            &mut quad, &texture, length, offset, front, back, !upper_peg,
        );
    }

    quad
}

fn gen_slope(
    start: wad::Vertex,
    end: wad::Vertex,
    front: f32,
    back: f32,
    height: f32,
) -> Quad {
    let start = Vec2::new(start.x, start.y);
    let end = Vec2::new(end.x, end.y);
    let mut quad = create_quad(start, end, front, back);

    let normal = quad.points[0].normal;

    if front < back {
        quad.points[1].pos += normal * height;
        quad.points[2].pos += normal * height;
    } else {
        quad.points[0].pos += normal * height;
        quad.points[3].pos += normal * height;
    }

    quad
}

pub fn gen_walls(
    context: &mut Context,
    wad_map: &wad::Map,
    wad_sector: &wad::Sector,
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

            if !linedef.flags.contains(wad::LinedefFlags::TWO_SIDED) {
                if let Some(sidedef) = linedef.front_sidedef {
                    let sidedef = &wad_map.sidedefs[sidedef];

                    let quad = create_normal_wall_quad(
                        context, wad_sector, &linedef, &sidedef, start, end,
                    );

                    quads.push(quad);
                }
            }

            if linedef.front_sidedef.is_some()
                && linedef.back_sidedef.is_some()
            {
                let front_sidedef = linedef.front_sidedef.unwrap();
                let front_sidedef = &wad_map.sidedefs[front_sidedef];

                let back_sidedef = linedef.back_sidedef.unwrap();
                let back_sidedef = &wad_map.sidedefs[back_sidedef];

                let front_sector = &wad_map.sectors[front_sidedef.sector];
                let back_sector = &wad_map.sectors[back_sidedef.sector];

                // Generate the floor difference
                if front_sector.floor_height != back_sector.floor_height {
                    let front = front_sector.floor_height;
                    let back = back_sector.floor_height;
                    let height = (front - back).abs();

                    if height <= 24.0 {
                        let quad = gen_slope(start, end, front, back, height);
                        slope_quads.push(quad);
                    }

                    let (texture_id, texture) =
                        if front_sidedef.lower_texture == "-" {
                            context.texture(&back_sidedef.lower_texture, true)
                        } else {
                            context.texture(&front_sidedef.lower_texture, true)
                        };

                    let mut quad = gen_diff_wall(
                        texture,
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
                    quad.texture_id = texture_id;

                    quads.push(quad);
                }

                // Generate the height difference
                if front_sector.ceiling_height != back_sector.ceiling_height {
                    let front = front_sector.ceiling_height;
                    let back = back_sector.ceiling_height;

                    let (texture_id, texture) =
                        if front_sidedef.upper_texture == "-" {
                            context.texture(&back_sidedef.upper_texture, true)
                        } else {
                            context.texture(&front_sidedef.upper_texture, true)
                        };

                    let mut quad = gen_diff_wall(
                        texture,
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
                    quad.texture_id = texture_id;

                    quads.push(quad);
                }
            }
        }
    }

    (quads, slope_quads)
}
