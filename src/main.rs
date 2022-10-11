use std::fs::File;
use std::io::BufWriter;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use clap::Parser;
use serde::{Serialize, Deserialize};

use wad::Wad;
use gltf::{Vec2, Vec3, Vec4, Vertex};

mod gltf;
mod util;
mod wad;

/// TODO(patrik):
///   - Lazy loading textures

struct TextureLoader {
    color_map: ColorMap,
    palette: Palette,

    textures: Vec<(String, Texture)>,
}

impl TextureLoader {
    fn new(wad: &Wad, color_map: ColorMap, palette: Palette) -> Option<Self> {
        assert!(!wad.find_dir("P3_START").is_ok());

        let mut result = Self {
            color_map,
            palette,

            textures: Vec::new(),
        };

        result.load_all_patches(wad);
        result.load_all_flats(wad);
        result.load_all_textures(wad);

        Some(result)
    }

    fn load_all_patches(&mut self, wad: &wad::Wad) {
        let start = wad.find_dir("P_START").unwrap();
        let start = start + 1;
        let end = wad.find_dir("P_END").unwrap();
        assert!(start < end);

        for index in start..end {
            // TODO(patrik): Remove unwarp
            let entry = wad.read_dir_entry(index).unwrap();

            let null_pos = entry
                .name
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.name.len());
            let entry_name = &entry.name[..null_pos];
            let entry_name = std::str::from_utf8(&entry_name)
                .expect("Failed to convert floor texture name to str");

            let skip = ["P1_START", "P1_END", "P2_START", "P2_END"]
                .iter()
                .any(|s| *s == entry_name);
            if skip {
                continue;
            }

            // TODO(patrik): Remove unwarp
            let texture = read_patch_texture(
                wad,
                entry_name,
                &self.color_map,
                &self.palette,
            )
            .unwrap();

            self.add_texture(entry_name, texture);
        }
    }

    fn load_all_flats(&mut self, wad: &wad::Wad) {
        let start = wad.find_dir("F_START").unwrap();
        let start = start + 1;
        let end = wad.find_dir("F_END").unwrap();
        assert!(start < end);

        for index in start..end {
            // TODO(patrik): Remove unwarp
            let entry = wad.read_dir_entry(index).unwrap();

            let null_pos = entry
                .name
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.name.len());
            let entry_name = &entry.name[..null_pos];
            let entry_name = std::str::from_utf8(&entry_name)
                .expect("Failed to convert floor texture name to str");

            let skip = ["F1_START", "F1_END", "F2_START", "F2_END"]
                .iter()
                .any(|s| *s == entry_name);
            if skip {
                continue;
            }

            // TODO(patrik): Remove unwarp
            let texture = read_flat_texture(
                wad,
                entry_name,
                &self.color_map,
                &self.palette,
            )
            .unwrap();

            self.add_texture(entry_name, texture);
        }
    }

    fn load_all_textures(&mut self, wad: &wad::Wad) {
        let patch_names =
            read_patch_names(&wad).expect("Failed to load patch names");

        let texture_defs =
            read_texture_defs(&wad).expect("Failed to read texture defs");

        let textures = process_texture_defs(
            &wad,
            self,
            &patch_names,
            &texture_defs,
            &self.color_map,
            &self.palette,
        );

        for (name, texture) in textures {
            self.add_texture(&name, texture);
        }
    }

    fn add_texture(&mut self, name: &str, texture: Texture) {
        if self.textures.iter().any(|t| t.0 == name) {
            // TODO(patrik): Check texture if they are the same?
            eprintln!("Warning: Duplicate texture '{}'", name);
            return;
        }

        self.textures.push((name.to_string(), texture));
    }

    fn load(&self, name: &str) -> Option<&Texture> {
        for (texture_name, texture) in &self.textures {
            if texture_name == name {
                return Some(texture);
            }
        }

        None
    }
}

struct TextureQueue {
    textures: Vec<String>,
}

impl TextureQueue {
    fn new() -> Self {
        Self {
            textures: Vec::new(),
        }
    }

    fn get(&self, name: &str) -> Option<usize> {
        for i in 0..self.textures.len() {
            if self.textures[i] == name {
                return Some(i);
            }
        }

        return None;
    }

    fn get_name_from_id(&self, id: usize) -> Option<&String> {
        self.textures.get(id)
    }

    fn enqueue(&mut self, name: String) -> Option<usize> {
        if name == "-" {
            return None;
        }

        // println!("Enqueing Texture: {:?}", name);
        return if let Some(index) = self.get(&name) {
            Some(index)
        } else {
            let id = self.textures.len();
            self.textures.push(name);

            Some(id)
        };
    }
}

struct Mesh {
    vertex_buffer: Vec<Vertex>,
    index_buffer: Vec<u32>,
    texture_id: Option<usize>,
}

impl Mesh {
    fn new() -> Self {
        Self {
            vertex_buffer: Vec::new(),
            index_buffer: Vec::new(),
            texture_id: None,
        }
    }

    fn add_vertices(&mut self, vertices: &[Vertex], clockwise: bool) {
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
struct Quad {
    points: [Vertex; 4],
    texture_id: usize,
}

impl Quad {
    fn new() -> Self {
        Self {
            points: [Default::default(); 4],
            texture_id: 0,
        }
    }
}

struct Sector {
    floor_mesh: Mesh,
    ceiling_mesh: Mesh,
    wall_quads: Vec<Quad>,
    slope_quads: Vec<Quad>,
}

impl Sector {
    fn new(
        floor_mesh: Mesh,
        ceiling_mesh: Mesh,
        wall_quads: Vec<Quad>,
        slope_quads: Vec<Quad>,
    ) -> Self {
        Self {
            floor_mesh,
            ceiling_mesh,
            wall_quads,
            slope_quads,
        }
    }
}

struct Map {
    sectors: Vec<Sector>,
    slope_mesh: Mesh,
}

impl Map {
    fn new(sectors: Vec<Sector>, slope_mesh: Mesh) -> Self {
        Self {
            sectors,
            slope_mesh,
        }
    }
}

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

fn generate_sector_floor(
    wad: &wad::Wad,
    map: &wad::Map,
    texture_loader: &TextureLoader,
    texture_queue: &mut TextureQueue,
    sector: &wad::Sector,
) -> Mesh {
    let mut mesh = Mesh::new();

    let texture_id = queue_texture(texture_queue, sector.floor_texture_name);
    mesh.texture_id = texture_id;

    let name = texture_queue.get_name_from_id(texture_id.unwrap()).unwrap();
    let texture = texture_loader.load(name).unwrap();

    let w = 1.0 / texture.width as f32;
    let h = 1.0 / texture.height as f32;

    let dim = Vec2::new(w, -h);

    for sub_sector in &sector.sub_sectors {
        let mut verts = Vec::new();

        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];
            let start = map.vertex(segment.start_vertex);

            let pos = Vec3::new(start.x, sector.floor_height, start.y);
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

fn generate_sector_ceiling(
    wad: &wad::Wad,
    map: &wad::Map,
    texture_loader: &TextureLoader,
    texture_queue: &mut TextureQueue,
    sector: &wad::Sector,
) -> Mesh {
    let mut mesh = Mesh::new();

    let texture_id = queue_texture(texture_queue, sector.floor_texture_name);
    mesh.texture_id = texture_id;

    let name = texture_queue.get_name_from_id(texture_id.unwrap()).unwrap();
    let texture = texture_loader.load(name).unwrap();

    let w = 1.0 / texture.width as f32;
    let h = 1.0 / texture.height as f32;

    let dim = Vec2::new(w, -h);

    for sub_sector in &sector.sub_sectors {
        let mut verts = Vec::new();

        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];
            let start = map.vertex(segment.start_vertex);

            let pos = Vec3::new(start.x, sector.ceiling_height, start.y);
            let uv = Vec2::new(start.x, start.y) * dim;
            let color = Vec4::new(1.0, 1.0, 1.0, 1.0);
            let normal = Vec3::new(0.0, -1.0, 0.0);
            verts.push(Vertex::new(pos, normal, uv, color));
        }

        util::cleanup_lines(&mut verts);
        mesh.add_vertices(&verts, false);
    }

    let texture_id = queue_texture(texture_queue, sector.ceiling_texture_name);
    mesh.texture_id = texture_id;

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

    if lower_peg {
        y2 = y_offset + texture.height as f32;
        y1 = y2 - height;
    }

    let dim = Vec2::new(texture.width as f32, texture.height as f32);

    quad.points[0].uv =
        Vec2::new(x_offset, y1 + (top - quad.points[0].pos.y)) / dim;
    quad.points[1].uv =
        Vec2::new(x_offset, y2 + (bottom - quad.points[1].pos.y)) / dim;
    quad.points[2].uv =
        Vec2::new(x_offset + length, y2 + (bottom - quad.points[2].pos.y))
            / dim;
    quad.points[3].uv =
        Vec2::new(x_offset + length, y1 + (top - quad.points[3].pos.y)) / dim;
}

fn create_normal_wall_quad(
    wad: &wad::Wad,
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
    wad: &wad::Wad,
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
    let pos0 = Vec3::new(start.x, front, start.y);
    let pos1 = Vec3::new(end.x, front, end.y);
    let pos2 = Vec3::new(end.x, back, end.y);
    let pos3 = Vec3::new(start.x, back, start.y);

    let a = pos1;
    let b = pos3;
    let c = pos2;

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

    quad.points[0].pos += normal * diff;
    quad.points[1].pos += normal * diff;

    quad
}

fn generate_sector_wall(
    wad: &wad::Wad,
    map: &wad::Map,
    texture_loader: &TextureLoader,
    texture_queue: &mut TextureQueue,
    sector: &wad::Sector,
    slope_mesh: &mut Mesh,
) -> (Vec<Quad>, Vec<Quad>) {
    let mut quads = Vec::new();
    let mut slope_quads = Vec::new();

    for sub_sector in &sector.sub_sectors {
        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];

            if segment.linedef != 0xffff {
                let linedef = map.linedefs[segment.linedef];
                let line = linedef.line;
                let start = map.vertex(line.start_vertex);
                let end = map.vertex(line.end_vertex);

                if linedef.flags & wad::LINEDEF_FLAG_TWO_SIDED
                    != wad::LINEDEF_FLAG_TWO_SIDED
                {
                    if let Some(sidedef) = linedef.front_sidedef {
                        let sidedef = map.sidedefs[sidedef];

                        let quad = create_normal_wall_quad(
                            wad,
                            texture_loader,
                            texture_queue,
                            sector,
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
                    let front_sidedef = map.sidedefs[front_sidedef];

                    let back_sidedef = linedef.back_sidedef.unwrap();
                    let back_sidedef = map.sidedefs[back_sidedef];

                    let front_sector = &map.sectors[front_sidedef.sector];
                    let back_sector = &map.sectors[back_sidedef.sector];

                    // Generate the floor difference
                    if front_sector.floor_height != back_sector.floor_height {
                        let front = front_sector.floor_height;
                        let back = back_sector.floor_height;

                        let min = front.min(back);
                        let max = front.max(back);

                        let diff = max - min;

                        if diff <= 24.0 {
                            let quad =
                                gen_slope(start, end, front, back, diff);
                            slope_quads.push(quad);
                        }

                        let texture_id = queue_texture(
                            texture_queue,
                            front_sidedef.lower_texture_name,
                        )
                        .unwrap_or(0);

                        let quad = gen_diff_wall(
                            wad,
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
                    if front_sector.ceiling_height
                        != back_sector.ceiling_height
                    {
                        let front = front_sector.ceiling_height;
                        let back = back_sector.ceiling_height;

                        let texture_id = queue_texture(
                            texture_queue,
                            front_sidedef.upper_texture_name,
                        )
                        .unwrap_or(0);

                        let quad = gen_diff_wall(
                            wad,
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
    }

    (quads, slope_quads)
}

fn generate_sector_from_wad(
    wad: &wad::Wad,
    map: &wad::Map,
    texture_loader: &TextureLoader,
    texture_queue: &mut TextureQueue,
    sector: &wad::Sector,
    slope_mesh: &mut Mesh,
) -> Sector {
    let floor_mesh =
        generate_sector_floor(wad, map, texture_loader, texture_queue, sector);
    let ceiling_mesh = generate_sector_ceiling(
        wad,
        map,
        texture_loader,
        texture_queue,
        sector,
    );

    let (wall_quads, slope_quads) = generate_sector_wall(
        wad,
        map,
        texture_loader,
        texture_queue,
        sector,
        slope_mesh,
    );

    Sector::new(floor_mesh, ceiling_mesh, wall_quads, slope_quads)
}

fn generate_3d_map(
    wad: &wad::Wad,
    texture_loader: &TextureLoader,
    texture_queue: &mut TextureQueue,
    map_name: &str,
) -> Map {
    // Construct an map with map from the wad
    let map = wad::Map::parse_from_wad(&wad, map_name)
        .expect("Failed to load wad map");

    let mut sectors = Vec::new();

    // let map_sector =
    //     generate_sector_from_wad(&map, texture_queue, &map.sectors[50]);
    // sectors.push(map_sector);

    let mut slope_mesh = Mesh::new();

    for sector in &map.sectors {
        let map_sector = generate_sector_from_wad(
            wad,
            &map,
            texture_loader,
            texture_queue,
            sector,
            &mut slope_mesh,
        );
        sectors.push(map_sector);
    }

    println!("Num Sectors: {}", sectors.len());

    Map::new(sectors, slope_mesh)
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The WAD file to convert
    wad_file: String,

    /// Write output file to <OUTPUT>
    #[clap(short, long)]
    output: Option<String>,

    /// Which map to convert (example E1M1)
    #[clap(short, long)]
    map: Option<String>,
}

#[derive(Copy, Clone, Default, Debug)]
struct PaletteColor {
    r: u8,
    g: u8,
    b: u8,
}

const MAX_PALETTE_COLORS: usize = 256;
const MAX_COLOR_MAPS: usize = 34;

const FLAT_TEXTURE_WIDTH: usize = 64;
const FLAT_TEXTURE_HEIGHT: usize = 64;

#[derive(Clone)]
struct Palette {
    colors: [PaletteColor; MAX_PALETTE_COLORS],
}

impl Palette {
    fn get(&self, index: usize) -> PaletteColor {
        self.colors[index]
    }
}

#[derive(Clone)]
struct ColorMap {
    map: [usize; MAX_PALETTE_COLORS],
}

impl ColorMap {
    fn get(&self, index: usize) -> usize {
        self.map[index]
    }

    fn get_color_from_palette(
        &self,
        palette: &Palette,
        index: usize,
    ) -> PaletteColor {
        let palette_index = self.get(index);
        palette.get(palette_index)
    }
}

fn read_all_palettes(wad: &Wad) -> Option<Vec<Palette>> {
    if let Ok(index) = wad.find_dir("PLAYPAL") {
        let playpal = wad.read_dir(index).expect("Failed to get PLAYPAL data");
        // One palette entry (R, G, B) 3 bytes
        let num_colors = playpal.len() / 3;
        // 256 palette entries per palette
        let palette_count = num_colors / MAX_PALETTE_COLORS;

        let mut palettes = Vec::new();

        for palette in 0..palette_count {
            let mut colors = [PaletteColor::default(); MAX_PALETTE_COLORS];

            let data_start = palette * (256 * 3);
            for color_index in 0..256 {
                let start = color_index * 3 + data_start;
                let r = playpal[start + 0];
                let g = playpal[start + 1];
                let b = playpal[start + 2];
                colors[color_index] = PaletteColor { r, g, b };
            }

            palettes.push(Palette { colors });
        }

        return Some(palettes);
    }

    None
}

fn read_all_color_maps(wad: &Wad) -> Option<Vec<ColorMap>> {
    if let Ok(index) = wad.find_dir("COLORMAP") {
        let color_map_table =
            wad.read_dir(index).expect("Failed to get COLORMAP data");

        let mut color_maps = Vec::with_capacity(MAX_COLOR_MAPS);

        for color_map_index in 0..MAX_COLOR_MAPS {
            let data_start = color_map_index * MAX_PALETTE_COLORS;
            let mut color_map = [0usize; MAX_PALETTE_COLORS];
            for index in 0..MAX_PALETTE_COLORS {
                let start = index + data_start;
                let palette_index = color_map_table[start] as usize;
                color_map[index] = palette_index;
            }

            color_maps.push(ColorMap { map: color_map });
        }

        return Some(color_maps);
    }

    None
}

#[derive(Clone)]
struct Texture {
    width: usize,
    height: usize,
    left_offset: i16,
    top_offset: i16,
    pixels: Vec<u8>,
}

fn read_flat_texture(
    wad: &Wad,
    name: &str,
    color_map: &ColorMap,
    palette: &Palette,
) -> Option<Texture> {
    if let Ok(index) = wad.find_dir(name) {
        let texture_data = wad.read_dir(index).ok()?;

        let mut pixels =
            vec![0u8; FLAT_TEXTURE_WIDTH * FLAT_TEXTURE_HEIGHT * 4];

        for x in 0..FLAT_TEXTURE_WIDTH {
            for y in 0..FLAT_TEXTURE_HEIGHT {
                let start = x + y * FLAT_TEXTURE_WIDTH;
                let index = texture_data[start];
                let index = index as usize;

                let color = color_map.get_color_from_palette(palette, index);

                let img_index = x + y * FLAT_TEXTURE_WIDTH;
                pixels[img_index * 4 + 0] = color.r;
                pixels[img_index * 4 + 1] = color.g;
                pixels[img_index * 4 + 2] = color.b;
                pixels[img_index * 4 + 3] = 0xffu8;
            }
        }

        return Some(Texture {
            width: FLAT_TEXTURE_WIDTH,
            height: FLAT_TEXTURE_HEIGHT,
            left_offset: 0,
            top_offset: 0,
            pixels,
        });
    }

    None
}

fn read_patch_texture(
    wad: &Wad,
    name: &str,
    color_map: &ColorMap,
    palette: &Palette,
) -> Option<Texture> {
    if let Ok(index) = wad.find_dir(name) {
        let texture_data = wad.read_dir(index).ok()?;

        let width = u16::from_le_bytes(texture_data[0..2].try_into().unwrap());
        let height =
            u16::from_le_bytes(texture_data[2..4].try_into().unwrap());

        let left_offset =
            i16::from_le_bytes(texture_data[4..6].try_into().unwrap());
        let top_offset =
            i16::from_le_bytes(texture_data[6..8].try_into().unwrap());

        let width = width as usize;
        let height = height as usize;

        let mut pixels = vec![0u8; width * height * 4];

        let start_offset = 8;
        for x in 0..width {
            let start = x * 4 + start_offset;
            let offset = u32::from_le_bytes(
                texture_data[start..start + 4].try_into().unwrap(),
            );
            let offset = offset as usize;

            let mut new_offset = offset;
            let mut y_offset = 0;
            loop {
                // TODO(patrik): Should we use topdelta to correct the offset
                // inside the pixel buffer
                let topdelta = texture_data[new_offset];
                if topdelta == 0xff {
                    break;
                }

                let length = texture_data[new_offset + 1];
                let length = length as usize;

                let start = new_offset + 2;
                for data_offset in 0..length {
                    let index = texture_data[start + data_offset];
                    let index = index as usize;

                    let color =
                        color_map.get_color_from_palette(palette, index);

                    let y = y_offset;
                    let img_index = x + y * width;
                    pixels[img_index * 4 + 0] = color.r;
                    pixels[img_index * 4 + 1] = color.g;
                    pixels[img_index * 4 + 2] = color.b;
                    pixels[img_index * 4 + 3] = 0xffu8;

                    y_offset += 1;
                }

                new_offset += length + 4;
            }
        }

        return Some(Texture {
            width,
            height,
            left_offset,
            top_offset,
            pixels,
        });
    }

    None
}

fn write_texture_to_png<P>(path: P, texture: &Texture)
where
    P: AsRef<Path>,
{
    let file = File::create(path).unwrap();
    let ref mut file_writer = BufWriter::new(file);

    let mut encoder = png::Encoder::new(
        file_writer,
        texture.width as u32,
        texture.height as u32,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&texture.pixels).unwrap();
}

fn read_patch_names(wad: &Wad) -> Option<Vec<String>> {
    if let Ok(index) = wad.find_dir("PNAMES") {
        let data = wad.read_dir(index).ok()?;

        // NOTE(patrik):
        // https://doomwiki.org/wiki/PNAMES
        // "All integers are 4 bytes long in x86-style little-endian order.
        // Their values can never exceed 231-1,
        // since Doom reads them as signed ints."
        let num_map_patches = u32::from_le_bytes(data[0..4].try_into().ok()?);
        let num_map_patches = num_map_patches as usize;

        let mut names = Vec::with_capacity(num_map_patches);

        let offset = 4;
        for i in 0..num_map_patches {
            const NAME_LENGTH: usize = 8;
            let start = i * NAME_LENGTH + offset;
            let end = start + NAME_LENGTH;

            let name = &data[start..end];

            // Find the first occurance of a null-terminator/0
            let null_pos =
                name.iter().position(|&c| c == 0).unwrap_or(name.len());

            // Name without the null-terminator
            let name = &name[..null_pos];

            // Convert to str
            let name = std::str::from_utf8(&name[..null_pos]).ok()?;

            // Add to the list
            // TODO(patrik): Think this is a bug?
            // Error because W94_1 was w94_1
            names.push(String::from(name).to_uppercase());
        }

        return Some(names);
    }

    None
}

#[derive(Copy, Clone, Debug)]
struct PatchDef {
    patch: usize,
    origin_x: i16,
    origin_y: i16,
}

#[derive(Clone, Debug)]
struct TextureDef {
    name: String,
    width: usize,
    height: usize,
    patches: Vec<PatchDef>,
}

fn process_texture_lump(
    wad: &wad::Wad,
    texture_defs: &mut Vec<TextureDef>,
    index: usize,
) -> Option<()> {
    let data = wad.read_dir(index).unwrap();

    let num_textures = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let num_textures = num_textures as usize;

    let data_offset = 4;
    for i in 0..num_textures {
        let start = i * 4 + data_offset;

        let offset =
            u32::from_le_bytes(data[start..start + 4].try_into().unwrap());
        let offset = offset as usize;

        let name = &data[offset + 0..offset + 8];
        let null_pos = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        let name = &name[..null_pos];
        let name = std::str::from_utf8(name).ok()?;
        let name = String::from(name);

        let _masked =
            u32::from_le_bytes(data[offset + 8..offset + 12].try_into().ok()?);

        let width = u16::from_le_bytes(
            data[offset + 12..offset + 14].try_into().ok()?,
        );
        let width = width as usize;
        let height = u16::from_le_bytes(
            data[offset + 14..offset + 16].try_into().ok()?,
        );
        let height = height as usize;

        let _column_directory = u32::from_le_bytes(
            data[offset + 16..offset + 20].try_into().ok()?,
        );

        let patch_count = u16::from_le_bytes(
            data[offset + 20..offset + 22].try_into().ok()?,
        );
        let patch_count = patch_count as usize;

        let mut patches = Vec::with_capacity(patch_count);

        let offset = 22 + offset;
        for pi in 0..patch_count {
            let start = pi * 10 + offset;

            let origin_x = i16::from_le_bytes(
                data[start + 0..start + 2].try_into().ok()?,
            );

            let origin_y = i16::from_le_bytes(
                data[start + 2..start + 4].try_into().ok()?,
            );

            let patch = u16::from_le_bytes(
                data[start + 4..start + 6].try_into().ok()?,
            );
            let patch = patch as usize;

            let _step_dir = u16::from_le_bytes(
                data[start + 6..start + 8].try_into().ok()?,
            );

            let _color_map = u16::from_le_bytes(
                data[start + 8..start + 10].try_into().ok()?,
            );

            patches.push(PatchDef {
                patch,
                origin_x,
                origin_y,
            });
        }

        texture_defs.push(TextureDef {
            name,
            width,
            height,
            patches,
        });
    }

    Some(())
}

fn read_texture_defs(wad: &Wad) -> Option<Vec<TextureDef>> {
    let mut texture_defs = Vec::new();

    if let Ok(index) = wad.find_dir("TEXTURE1") {
        process_texture_lump(wad, &mut texture_defs, index)?;
    }

    if let Ok(index) = wad.find_dir("TEXTURE2") {
        process_texture_lump(wad, &mut texture_defs, index)?;
    }

    Some(texture_defs)
}

fn process_texture_defs(
    wad: &Wad,
    texture_loader: &TextureLoader,
    patch_names: &Vec<String>,
    texture_defs: &Vec<TextureDef>,
    color_map: &ColorMap,
    palette: &Palette,
) -> HashMap<String, Texture> {
    let mut result = HashMap::new();

    for def in texture_defs {
        let mut pixels = vec![0u8; def.width * def.height * 4];
        for patch in &def.patches {
            let patch_name = &patch_names[patch.patch];

            // let texture =
            //     read_patch_texture(wad, &patch_name, color_map, palette)
            //         .expect("Failed to read patch texture");

            let texture = texture_loader
                .load(&patch_name)
                .expect("Failed to read patch texture");

            let xoff = patch.origin_x as isize;
            let yoff = patch.origin_y as isize;
            for sy in 0..texture.height {
                for sx in 0..texture.width {
                    let source_index = sx + sy * texture.width;

                    let x = sx as isize + xoff;
                    let y = sy as isize + yoff;

                    if x < 0 || y < 0 {
                        continue;
                    }

                    if x >= def.width as isize || y >= def.height as isize {
                        continue;
                    }

                    let dest_index = (x as usize) + (y as usize) * def.width;

                    pixels[dest_index * 4 + 0] =
                        texture.pixels[source_index * 4 + 0];
                    pixels[dest_index * 4 + 1] =
                        texture.pixels[source_index * 4 + 1];
                    pixels[dest_index * 4 + 2] =
                        texture.pixels[source_index * 4 + 2];
                    pixels[dest_index * 4 + 3] =
                        texture.pixels[source_index * 4 + 3];
                }
            }
        }

        let new_texture = Texture {
            width: def.width,
            height: def.height,
            left_offset: 0,
            top_offset: 0,
            pixels,
        };

        result.insert(def.name.clone(), new_texture);
    }

    result
}

fn write_all_textures(textures: &HashMap<String, Texture>) {
    for (name, texture) in textures {
        let path = format!("test/{}.png", name);
        write_texture_to_png(&path, texture);
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfAccessor {
    buffer_view: usize,
    component_type: usize,
    count: usize,
    #[serde(rename = "type")]
    typ: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfSampler {
    name: String,
    mag_filter: usize,
    min_filter: usize,
    wrap_s: usize,
    wrap_t: usize,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfTexture {
    name: String,
    sampler: usize,
    source: usize,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfAsset {
    generator: String,
    version: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfBufferView {
    buffer: usize,
    byte_length: usize,
    byte_offset: usize,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfBuffer {
    byte_length: usize,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfTextureInfo {
    index: usize,
    tex_coord: usize,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfPbr {
    base_color_factor: [f32; 4],
    base_color_texture: Option<GltfTextureInfo>,
    metallic_factor: f32,
    roughness_factor: f32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfMaterial {
    name: String,
    double_sided: bool,
    pbr_metallic_roughness: GltfPbr,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfPrimitive {
    mode: usize,
    attributes: HashMap<String, usize>,
    indices: usize,
    material: usize,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfMesh {
    name: String,
    primitives: Vec<GltfPrimitive>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfNode {
    name: String,
    mesh: usize,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfScene {
    name: String,
    nodes: Vec<usize>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfImage {
    name: String,
    mime_type: String,
    buffer_view: usize,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GltfJson {
    accessors: Vec<GltfAccessor>,
    asset: GltfAsset,
    buffer_views: Vec<GltfBufferView>,
    buffers: Vec<GltfBuffer>,
    materials: Vec<GltfMaterial>,
    meshes: Vec<GltfMesh>,
    nodes: Vec<GltfNode>,
    scene: usize,
    scenes: Vec<GltfScene>,
    samplers: Vec<GltfSampler>,
    images: Vec<GltfImage>,
    textures: Vec<GltfTexture>,
}

type BufferViewId = usize;
type MaterialId = usize;
type AccessorId = usize;
type SamplerId = usize;
type TextureId = usize;
type SceneId = usize;
type ImageId = usize;
type MeshId = usize;
type NodeId = usize;

#[derive(Copy, Clone, PartialEq, Debug)]
enum DataTyp {
    Uint32,
    Vec2f,
    Vec3f,
    Vec4f,
}

struct Gltf {
    data_buffer: Vec<u8>,
    buffer_views: Vec<GltfBufferView>,
    materials: Vec<GltfMaterial>,
    accessors: Vec<GltfAccessor>,
    samplers: Vec<GltfSampler>,
    textures: Vec<GltfTexture>,
    scenes: Vec<GltfScene>,
    images: Vec<GltfImage>,
    meshes: Vec<GltfMesh>,
    nodes: Vec<GltfNode>,
}

impl Gltf {
    fn new() -> Self {
        Self {
            data_buffer: Vec::new(),
            buffer_views: Vec::new(),
            materials: Vec::new(),
            accessors: Vec::new(),
            samplers: Vec::new(),
            textures: Vec::new(),
            scenes: Vec::new(),
            images: Vec::new(),
            meshes: Vec::new(),
            nodes: Vec::new(),
        }
    }

    fn create_sampler(&mut self, name: String) -> SamplerId {
        let id = self.samplers.len();

        const NEAREST: usize = 9984;

        const REPEAT: usize = 10497;

        let sampler = GltfSampler {
            name,
            mag_filter: NEAREST,
            min_filter: NEAREST,
            wrap_s: REPEAT,
            wrap_t: REPEAT,
        };

        self.samplers.push(sampler);
        id
    }

    fn create_image(&mut self, name: String, data: &[u8]) -> ImageId {
        let id = self.images.len();

        let start = self.data_buffer.len();
        self.data_buffer.extend_from_slice(data);
        let end = self.data_buffer.len();

        let length = end - start;

        let buffer_view = self.create_buffer_view(start, length);

        let image = GltfImage {
            name,
            mime_type: "image/png".to_string(),
            buffer_view,
        };

        self.images.push(image);
        id
    }

    fn create_texture(
        &mut self,
        name: String,
        sampler_id: SamplerId,
        image_id: ImageId,
    ) -> TextureId {
        let id = self.textures.len();

        let texture = GltfTexture {
            name,
            sampler: sampler_id,
            source: image_id,
        };
        self.textures.push(texture);

        id
    }

    fn create_material(
        &mut self,
        name: String,
        color: Vec4,
        texture: Option<GltfTextureInfo>,
    ) -> MaterialId {
        let id = self.materials.len();
        let material = GltfMaterial {
            name,
            double_sided: false,
            pbr_metallic_roughness: GltfPbr {
                base_color_factor: [color.x, color.y, color.z, color.w],
                base_color_texture: texture,
                metallic_factor: 0.0,
                roughness_factor: 1.0,
            },
        };

        self.materials.push(material);
        id
    }

    fn create_mesh(&mut self, name: String) -> MeshId {
        let id = self.meshes.len();
        let mesh = GltfMesh {
            name,
            primitives: Vec::new(),
        };

        self.meshes.push(mesh);
        id
    }

    fn create_buffer_view(
        &mut self,
        start: usize,
        length: usize,
    ) -> BufferViewId {
        let id = self.buffer_views.len();
        let buffer_view = GltfBufferView {
            buffer: 0,
            byte_length: length,
            byte_offset: start,
        };

        self.buffer_views.push(buffer_view);
        id
    }

    fn add_vertex_buffer(&mut self, vertices: &[Vec3]) -> BufferViewId {
        let start = self.data_buffer.len();

        for vertex in vertices {
            let x = vertex.x / 20.0;
            let y = vertex.y / 20.0;
            let z = vertex.z / 20.0;

            self.data_buffer.extend_from_slice(&x.to_le_bytes());
            self.data_buffer.extend_from_slice(&y.to_le_bytes());
            self.data_buffer.extend_from_slice(&z.to_le_bytes());
        }

        let end = self.data_buffer.len();

        let length = end - start;

        self.create_buffer_view(start, length)
    }

    fn add_normal_buffer(&mut self, normals: &[Vec3]) -> BufferViewId {
        let start = self.data_buffer.len();

        for normal in normals {
            self.data_buffer.extend_from_slice(&normal.x.to_le_bytes());
            self.data_buffer.extend_from_slice(&normal.y.to_le_bytes());
            self.data_buffer.extend_from_slice(&normal.z.to_le_bytes());
        }

        let end = self.data_buffer.len();

        let length = end - start;

        self.create_buffer_view(start, length)
    }

    fn add_uv_buffer(&mut self, uvs: &[Vec2]) -> BufferViewId {
        let start = self.data_buffer.len();

        for uv in uvs {
            let u = uv.x;
            let v = uv.y;
            self.data_buffer.extend_from_slice(&u.to_le_bytes());
            self.data_buffer.extend_from_slice(&v.to_le_bytes());
        }

        let end = self.data_buffer.len();

        let length = end - start;

        self.create_buffer_view(start, length)
    }

    fn add_color_buffer(&mut self, colors: &[Vec4]) -> BufferViewId {
        let start = self.data_buffer.len();

        for color in colors {
            self.data_buffer.extend_from_slice(&color.x.to_le_bytes());
            self.data_buffer.extend_from_slice(&color.y.to_le_bytes());
            self.data_buffer.extend_from_slice(&color.z.to_le_bytes());
            self.data_buffer.extend_from_slice(&color.w.to_le_bytes());
        }

        let end = self.data_buffer.len();

        let length = end - start;

        self.create_buffer_view(start, length)
    }

    fn add_index_buffer(&mut self, indices: &[u32]) -> BufferViewId {
        let start = self.data_buffer.len();

        for index in indices {
            self.data_buffer.extend_from_slice(&index.to_le_bytes())
        }

        let end = self.data_buffer.len();

        let length = end - start;

        self.create_buffer_view(start, length)
    }

    fn create_accessor(
        &mut self,
        buffer_view_id: BufferViewId,
        count: usize,
        data_typ: DataTyp,
    ) -> AccessorId {
        let id = self.accessors.len();

        // NOTE(patrik): From GLAD OpenGL Loader headers
        const GL_UNSIGNED_INT: usize = 0x1405;
        const GL_FLOAT: usize = 0x1406;

        let (component_type, typ) = match data_typ {
            DataTyp::Uint32 => (GL_UNSIGNED_INT, "SCALAR"),
            DataTyp::Vec2f => (GL_FLOAT, "VEC2"),
            DataTyp::Vec3f => (GL_FLOAT, "VEC3"),
            DataTyp::Vec4f => (GL_FLOAT, "VEC4"),
        };

        let accessor = GltfAccessor {
            buffer_view: buffer_view_id,
            component_type,
            count,
            typ: typ.to_string(),
        };
        self.accessors.push(accessor);

        id
    }

    fn add_mesh_primitive(
        &mut self,
        mesh_id: MeshId,
        mesh: &Mesh,
        material_id: MaterialId,
    ) {
        let pos = mesh
            .vertex_buffer
            .iter()
            .map(|v| v.pos)
            .collect::<Vec<Vec3>>();
        let vertex_buffer_view = self.add_vertex_buffer(&pos);
        let vertex_buffer_access = self.create_accessor(
            vertex_buffer_view,
            pos.len(),
            DataTyp::Vec3f,
        );

        let normals = mesh
            .vertex_buffer
            .iter()
            .map(|v| v.normal)
            .collect::<Vec<Vec3>>();
        let normal_buffer_view = self.add_normal_buffer(&normals);
        let normal_buffer_access = self.create_accessor(
            normal_buffer_view,
            normals.len(),
            DataTyp::Vec3f,
        );

        let uvs = mesh
            .vertex_buffer
            .iter()
            .map(|v| v.uv)
            .collect::<Vec<Vec2>>();
        let uv_buffer_view = self.add_uv_buffer(&uvs);
        let uv_buffer_view =
            self.create_accessor(uv_buffer_view, uvs.len(), DataTyp::Vec2f);

        let colors = mesh
            .vertex_buffer
            .iter()
            .map(|v| v.color)
            .collect::<Vec<Vec4>>();
        let color_buffer_view = self.add_color_buffer(&colors);
        let color_buffer_access = self.create_accessor(
            color_buffer_view,
            colors.len(),
            DataTyp::Vec4f,
        );

        let index_buffer_view = self.add_index_buffer(&mesh.index_buffer);
        let index_buffer_access = self.create_accessor(
            index_buffer_view,
            mesh.index_buffer.len(),
            DataTyp::Uint32,
        );

        let mut attributes = HashMap::new();
        attributes.insert("POSITION".to_string(), vertex_buffer_access);
        attributes.insert("NORMAL".to_string(), normal_buffer_access);
        attributes.insert("TEXCOORD_0".to_string(), uv_buffer_view);
        attributes.insert("COLOR_0".to_string(), color_buffer_access);

        let primitive = GltfPrimitive {
            mode: 4,
            attributes,
            indices: index_buffer_access,
            material: material_id,
        };

        self.meshes[mesh_id].primitives.push(primitive);
    }

    fn create_node(&mut self, name: String, mesh_id: MeshId) -> NodeId {
        let id = self.nodes.len();
        let node = GltfNode {
            name,
            mesh: mesh_id,
        };

        self.nodes.push(node);
        id
    }

    fn create_scene(&mut self, name: String) -> SceneId {
        let id = self.scenes.len();
        let scene = GltfScene {
            name,
            nodes: Vec::new(),
        };

        self.scenes.push(scene);
        id
    }

    fn add_node_to_scene(&mut self, scene_id: SceneId, node_id: NodeId) {
        self.scenes[scene_id].nodes.push(node_id);
    }

    fn write_model(self) -> Vec<u8> {
        let buffer = GltfBuffer {
            byte_length: self.data_buffer.len(),
        };

        let asset = GltfAsset {
            generator: "Testing".to_string(),
            version: "2.0".to_string(),
        };

        let gltf_json = GltfJson {
            accessors: self.accessors,
            asset,
            buffer_views: self.buffer_views,
            buffers: vec![buffer],
            materials: self.materials,
            meshes: self.meshes,
            nodes: self.nodes,
            scene: 0,
            scenes: self.scenes,
            samplers: self.samplers,
            images: self.images,
            textures: self.textures,
        };

        // let text = serde_json::to_string_pretty(&gltf_json).unwrap();
        // println!("{}", text);

        let mut text = serde_json::to_string(&gltf_json).unwrap();
        // TODO(patrik): Fix?
        let padding = text.as_bytes().len() % 4;
        for _ in 0..(4 - padding) {
            text.push(' ');
        }

        assert_eq!(text.len(), text.as_bytes().len());

        let mut bin_buffer: Vec<u8> = Vec::new();
        bin_buffer.extend_from_slice(&0x46546c67u32.to_le_bytes());
        bin_buffer.extend_from_slice(&2u32.to_le_bytes());
        bin_buffer.extend_from_slice(&0u32.to_le_bytes());

        // JSON Chunk
        let data = text.as_bytes();
        bin_buffer.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bin_buffer.extend_from_slice(&0x4e4f534au32.to_le_bytes());
        bin_buffer.extend_from_slice(data);

        // Binary Buffer Chunk
        bin_buffer
            .extend_from_slice(&(self.data_buffer.len() as u32).to_le_bytes());
        bin_buffer.extend_from_slice(&0x004e4942u32.to_le_bytes());
        bin_buffer.extend_from_slice(&self.data_buffer);

        let total_size = bin_buffer.len() as u32;
        bin_buffer[8..12].copy_from_slice(&total_size.to_le_bytes());

        bin_buffer
    }
}

fn write_map_gltf<P>(
    wad: &Wad,
    map: Map,
    texture_queue: &TextureQueue,
    texture_loader: &TextureLoader,
    output_file: P,
) where
    P: AsRef<Path>,
{
    let mut gltf = Gltf::new();

    let map_name = "E1M1";

    let scene_id = gltf.create_scene(map_name.to_string());
    let texture_sampler = gltf.create_sampler("Default Sampler".to_string());

    let mut textures = Vec::new();
    for t in &texture_queue.textures {
        if let Some(texture) = texture_loader.load(&t) {
            println!("{}: {}, {}", t, texture.width, texture.height);

            let mut result = Vec::new();
            {
                let ref mut file_writer = BufWriter::new(&mut result);

                let mut encoder = png::Encoder::new(
                    file_writer,
                    texture.width as u32,
                    texture.height as u32,
                );
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);

                let mut writer = encoder.write_header().unwrap();
                writer.write_image_data(&texture.pixels).unwrap();
            }

            let image_id = gltf.create_image(t.clone(), &result);
            let texture_id =
                gltf.create_texture(t.clone(), texture_sampler, image_id);

            textures.push(texture_id);
        } else {
            panic!("Failed to load texture: '{}'", t);
        }
    }

    for sector_index in 0..map.sectors.len() {
        let sector = &map.sectors[sector_index];

        let mesh_id = gltf.create_mesh(format!("Sector #{}", sector_index));

        let material_id = gltf.create_material(
            format!("Sector #{} Floor", sector_index),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            Some(GltfTextureInfo {
                index: textures[sector.floor_mesh.texture_id.unwrap_or(0)],
                tex_coord: 0,
            }),
        );

        gltf.add_mesh_primitive(mesh_id, &sector.floor_mesh, material_id);

        let material_id = gltf.create_material(
            format!("Sector #{} Ceiling", sector_index),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            Some(GltfTextureInfo {
                index: textures[sector.ceiling_mesh.texture_id.unwrap_or(0)],
                tex_coord: 0,
            }),
        );

        gltf.add_mesh_primitive(mesh_id, &sector.ceiling_mesh, material_id);

        let mut wall_meshes: HashMap<usize, Mesh> = HashMap::new();
        for quad in &sector.wall_quads {
            let mesh =
                if let Some(mesh) = wall_meshes.get_mut(&quad.texture_id) {
                    mesh
                } else {
                    wall_meshes.insert(quad.texture_id, Mesh::new());
                    wall_meshes.get_mut(&quad.texture_id).unwrap()
                };

            mesh.add_vertices(&quad.points, false);
        }

        for (texture_id, mesh) in wall_meshes {
            let material_id = gltf.create_material(
                format!("Sector #{} Walls Tex #{}", sector_index, texture_id),
                Vec4::new(1.0, 1.0, 1.0, 1.0),
                Some(GltfTextureInfo {
                    index: textures[texture_id],
                    tex_coord: 0,
                }),
            );

            gltf.add_mesh_primitive(mesh_id, &mesh, material_id);
        }

        let node_id =
            gltf.create_node(format!("Sector #{}-col", sector_index), mesh_id);

        gltf.add_node_to_scene(scene_id, node_id);

        let slope_mesh_id =
            gltf.create_mesh(format!("Sector #{}: Slope Mesh", sector_index));

        let mut slope_mesh = Mesh::new();
        for quad in &sector.slope_quads {
            slope_mesh.add_vertices(&quad.points, false);
        }
        gltf.add_mesh_primitive(slope_mesh_id, &slope_mesh, 0);

        let extra_node_id = gltf.create_node(
            format!("Sector #{}: Slope Mesh-colonly", sector_index),
            slope_mesh_id,
        );
        gltf.add_node_to_scene(scene_id, extra_node_id);
    }

    let data = gltf.write_model();
    let mut file = File::create(output_file).unwrap();
    file.write_all(&data).unwrap();
}

fn main() {
    let args = Args::parse();
    println!("Args: {:?}", args);

    let output = if let Some(output) = args.output {
        PathBuf::from(output)
    } else {
        let mut path = PathBuf::from(args.wad_file.clone());
        path.set_extension("glb");
        path
    };

    // Read the raw wad file
    let data = util::read_binary_file(args.wad_file);
    // Parse the wad
    let wad = Wad::parse(&data).expect("Failed to parse WAD file");

    let palettes = read_all_palettes(&wad).expect("Failed to read palettes");
    let final_palette = &palettes[0];

    let color_maps =
        read_all_color_maps(&wad).expect("Failed to read color maps");
    let final_color_map = &color_maps[0];

    let texture_loader = TextureLoader::new(
        &wad,
        final_color_map.clone(),
        final_palette.clone(),
    )
    .expect("Failed to create TextureLoader");

    // texture_loader.debug_write_textures();

    let map = if let Some(map) = args.map.as_ref() {
        map.as_str()
    } else {
        // TODO(patrik): If args.map is none then we should convert all
        // the maps
        "E1M1"
    };

    println!("Converting '{}' to mime map", map);

    let mut texture_queue = TextureQueue::new();

    let map = generate_3d_map(&wad, &texture_loader, &mut texture_queue, map);
    write_map_gltf(&wad, map, &texture_queue, &texture_loader, output);
}
