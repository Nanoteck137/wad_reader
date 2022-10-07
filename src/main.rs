use std::fs::File;
use std::io::BufWriter;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use clap::Parser;
use serde::{Serialize, Deserialize};
use serde_json::Value;

use wad::Wad;
use gltf::{Vec2, Vec3, Vec4, Vertex};

mod gltf;
mod util;
mod wad;

/// TODO(patrik):
///   - Map format
///     - Textures

static COLOR_TABLE: [Vec4; 10] = [
    Vec4::new(
        0.6705882352941176,
        0.56078431372549020,
        0.564705882352941200,
        1.0,
    ),
    Vec4::new(
        0.7137254901960784,
        0.53333333333333330,
        0.223529411764705900,
        1.0,
    ),
    Vec4::new(
        0.6705882352941176,
        0.71372549019607840,
        0.686274509803921600,
        1.0,
    ),
    Vec4::new(
        0.9058823529411765,
        0.55686274509803920,
        0.725490196078431300,
        1.0,
    ),
    Vec4::new(
        0.4823529411764706,
        0.30196078431372547,
        0.396078431372549000,
        1.0,
    ),
    Vec4::new(
        0.4039215686274510,
        0.88627450980392150,
        0.027450980392156862,
        1.0,
    ),
    Vec4::new(
        0.6745098039215687,
        0.32941176470588235,
        0.078431372549019600,
        1.0,
    ),
    Vec4::new(
        0.9411764705882353,
        0.68235294117647060,
        0.843137254901960800,
        1.0,
    ),
    Vec4::new(
        0.7176470588235294,
        0.32941176470588235,
        0.156862745098039200,
        1.0,
    ),
    Vec4::new(
        0.6274509803921569,
        0.31372549019607840,
        0.011764705882352941,
        1.0,
    ),
];

fn read_file<P>(path: P) -> Vec<u8>
where
    P: AsRef<Path>,
{
    let mut file = File::open(path).unwrap();

    let mut result = Vec::new();
    file.read_to_end(&mut result).unwrap();

    result
}

struct TextureLoader {
    color_map: ColorMap,
    palette: Palette,

    patch_start: usize,
    patch_end: usize,
    flat_start: usize,
    flat_end: usize,
}

impl TextureLoader {
    fn new(wad: &Wad, color_map: ColorMap, palette: Palette) -> Option<Self> {
        assert!(!wad.find_dir("P3_START").is_ok());

        let patch_start = wad.find_dir("P_START").ok()?;
        let patch_end = wad.find_dir("P_END").ok()?;
        assert!(patch_start < patch_end);

        let flat_start = wad.find_dir("F_START").ok()?;
        let flat_end = wad.find_dir("F_END").ok()?;
        assert!(flat_start < flat_end);

        Some(Self {
            color_map,
            palette,

            patch_start,
            patch_end,
            flat_start,
            flat_end,
        })
    }

    fn is_patch(&self, index: usize) -> bool {
        index > self.patch_start && index < self.patch_end
    }

    fn is_flat(&self, index: usize) -> bool {
        index > self.flat_start && index < self.flat_end
    }

    fn load(&self, wad: &Wad, name: &str) -> Option<Texture> {
        let dir_index = wad.find_dir(name).ok()?;

        if self.is_patch(dir_index) {
            println!("{} is patch", name);
        } else if self.is_flat(dir_index) {
            println!("{} is flat", name);
            return read_flat_texture(
                wad,
                name,
                &self.color_map,
                &self.palette,
            );
        } else {
            panic!("{} is not flat or patch", name);
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

    fn enqueue(&mut self, name: String) -> usize {
        println!("Enqueing Texture: {:?}", name);
        return if let Some(index) = self.get(&name) {
            index
        } else {
            let id = self.textures.len();
            self.textures.push(name);

            id
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

    fn add_vertices(
        &mut self,
        mut vertices: Vec<Vertex>,
        clockwise: bool,
        cleanup: bool,
    ) {
        if cleanup {
            util::cleanup_lines(&mut vertices);
        }

        let triangles = util::triangulate(&vertices, clockwise);

        let index_offset = self.vertex_buffer.len();

        for v in &vertices {
            self.vertex_buffer.push(v.clone());
        }

        for i in &triangles {
            self.index_buffer.push(i + index_offset as u32);
        }
    }
}

struct Sector {
    floor_mesh: Mesh,
    ceiling_mesh: Mesh,
    wall_mesh: Mesh,
}

impl Sector {
    fn new(floor_mesh: Mesh, ceiling_mesh: Mesh, wall_mesh: Mesh) -> Self {
        Self {
            floor_mesh,
            ceiling_mesh,
            wall_mesh,
        }
    }
}

struct Map {
    sectors: Vec<Sector>,
}

impl Map {
    fn new(sectors: Vec<Sector>) -> Self {
        Self { sectors }
    }
}

fn generate_sector_floor(
    map: &wad::Map,
    texture_queue: &mut TextureQueue,
    sector: &wad::Sector,
) -> Mesh {
    let mut mesh = Mesh::new();

    let mut index = 0;

    for sub_sector in &sector.sub_sectors {
        let mut vertices = Vec::new();

        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];
            let start = map.vertex(segment.start_vertex);

            let pos = Vec3::new(start.x, sector.floor_height, start.y);
            let uv = Vec2::new(start.x, start.y);
            let color = COLOR_TABLE[index];
            vertices.push(Vertex::new(pos, uv, color));
        }

        index += 1;
        if index >= COLOR_TABLE.len() {
            index = 0;
        }

        mesh.add_vertices(vertices, true, true);
    }

    let texture_name = sector.floor_texture_name;
    let null_pos = texture_name
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(texture_name.len());
    let texture_name = &texture_name[..null_pos];
    let texture_name = std::str::from_utf8(&texture_name)
        .expect("Failed to convert floor texture name to str");
    let texture_name = String::from(texture_name);

    let texture_id = texture_queue.enqueue(texture_name);

    mesh.texture_id = Some(texture_id);

    mesh
}

fn generate_sector_ceiling(map: &wad::Map, sector: &wad::Sector) -> Mesh {
    let mut mesh = Mesh::new();

    let mut index = 0;

    for sub_sector in &sector.sub_sectors {
        let mut vertices = Vec::new();

        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];
            let start = map.vertex(segment.start_vertex);

            let pos = Vec3::new(start.x, sector.ceiling_height, start.y);
            let uv = Vec2::new(start.x, start.y);
            let color = COLOR_TABLE[index];
            vertices.push(Vertex::new(pos, uv, color));
        }

        index += 1;
        if index >= COLOR_TABLE.len() {
            index = 0;
        }

        mesh.add_vertices(vertices, false, true);
    }

    mesh
}

fn generate_sector_wall(map: &wad::Map, sector: &wad::Sector) -> Mesh {
    let mut mesh = Mesh::new();

    let mut index = 0;
    for sub_sector in &sector.sub_sectors {
        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];

            if segment.linedef != 0xffff {
                let mut wall = Vec::new();
                let linedef = map.linedefs[segment.linedef];
                let line = linedef.line;
                let start = map.vertex(line.start_vertex);
                let end = map.vertex(line.end_vertex);

                let color = COLOR_TABLE[index];

                if linedef.flags & wad::LINEDEF_FLAG_IMPASSABLE
                    == wad::LINEDEF_FLAG_IMPASSABLE
                    && linedef.flags & wad::LINEDEF_FLAG_TWO_SIDED
                        != wad::LINEDEF_FLAG_TWO_SIDED
                {
                    let dx = (end.x - start.x).abs();
                    let dy = (end.y - start.y).abs();

                    // Normalize the "vector"
                    let mag = (dx * dx + dy * dy).sqrt();
                    let dx = dx / mag;
                    let dy = dy / mag;

                    // TODO(patrik): We might need to revisit this and change
                    // the order
                    let uvs = if dx > dy {
                        [
                            Vec2::new(start.x, sector.floor_height),
                            Vec2::new(end.x, sector.floor_height),
                            Vec2::new(end.x, sector.ceiling_height),
                            Vec2::new(start.x, sector.ceiling_height),
                        ]
                    } else {
                        [
                            Vec2::new(end.y, sector.floor_height),
                            Vec2::new(start.y, sector.floor_height),
                            Vec2::new(start.y, sector.ceiling_height),
                            Vec2::new(end.y, sector.ceiling_height),
                        ]
                    };

                    let pos = Vec3::new(start.x, sector.floor_height, start.y);
                    let uv = uvs[0]; // 3
                    wall.push(Vertex::new(pos, uv, color));

                    let pos = Vec3::new(end.x, sector.floor_height, end.y);
                    let uv = uvs[1]; // 0
                    wall.push(Vertex::new(pos, uv, color));

                    let pos = Vec3::new(end.x, sector.ceiling_height, end.y);
                    let uv = uvs[2]; // 1
                    wall.push(Vertex::new(pos, uv, color));

                    let pos =
                        Vec3::new(start.x, sector.ceiling_height, start.y);
                    let uv = uvs[3]; // 2
                    wall.push(Vertex::new(pos, uv, color));
                }

                mesh.add_vertices(wall, false, false);

                let mut generate_wall = |front, back, clockwise| {
                    let mut verts = Vec::new();

                    let color = COLOR_TABLE[index];

                    let dx = (end.x - start.x).abs();
                    let dy = (end.y - start.y).abs();

                    // Normalize the "vector"
                    let mag = (dx * dx + dy * dy).sqrt();
                    let dx = dx / mag;
                    let dy = dy / mag;

                    // TODO(patrik): We might need to revisit this and change
                    // the order
                    let uvs = if dx > dy {
                        [
                            Vec2::new(start.x, front),
                            Vec2::new(end.x, front),
                            Vec2::new(end.x, back),
                            Vec2::new(start.x, back),
                        ]
                    } else {
                        [
                            Vec2::new(end.y, front),
                            Vec2::new(start.y, front),
                            Vec2::new(start.y, back),
                            Vec2::new(end.y, back),
                        ]
                    };

                    let pos0 = Vec3::new(start.x, front, start.y);
                    let pos2 = Vec3::new(end.x, front, end.y);
                    let pos1 = Vec3::new(end.x, back, end.y);

                    let normal = ((pos1 - pos0) * (pos2 - pos0)).normalize();
                    println!(
                        "Normal: {}, {}, {}",
                        normal.x, normal.y, normal.z
                    );

                    let pos = Vec3::new(start.x, front, start.y);
                    let uv = uvs[0];
                    verts.push(Vertex::new(pos, uv, color));

                    let pos = Vec3::new(end.x, front, end.y);
                    let uv = uvs[1];
                    verts.push(Vertex::new(pos, uv, color));

                    let pos = Vec3::new(end.x, back, end.y);
                    let uv = uvs[2];
                    verts.push(Vertex::new(pos, uv, color));

                    let pos = Vec3::new(start.x, back, start.y);
                    let uv = uvs[3];
                    verts.push(Vertex::new(pos, uv, color));

                    index += 1;
                    if index >= COLOR_TABLE.len() {
                        index = 0;
                    }

                    verts
                };

                let generate_slope = |front, back, diff| {
                    let mut verts = Vec::new();

                    let pos1 = Vec3::new(end.x, front, end.y);
                    let pos2 = Vec3::new(end.x, back, end.y);
                    let pos3 = Vec3::new(start.x, back, start.y);

                    let a = pos1;
                    let b = pos3;
                    let c = pos2;

                    let normal = ((b - a).cross(c - a)).normalize();

                    let x = (normal.x * 0.5) + 0.5;
                    let y = (normal.y * 0.5) + 0.5;
                    let z = (normal.z * 0.5) + 0.5;
                    let color = Vec4::new(x, y, z, 1.0);

                    let pos =
                        Vec3::new(start.x, front, start.y) + normal * diff;
                    let uv = Vec2::new(0.0, 0.0);
                    verts.push(Vertex::new(pos, uv, color));

                    let pos = Vec3::new(end.x, front, end.y) + normal * diff;
                    let uv = Vec2::new(0.0, 0.0);
                    verts.push(Vertex::new(pos, uv, color));

                    let pos = Vec3::new(end.x, back, end.y);
                    let uv = Vec2::new(0.0, 0.0);
                    verts.push(Vertex::new(pos, uv, color));

                    let pos = Vec3::new(start.x, back, start.y);
                    let uv = Vec2::new(0.0, 0.0);
                    verts.push(Vertex::new(pos, uv, color));

                    verts
                };

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
                            let verts = generate_slope(front, back, diff);
                            mesh.add_vertices(verts, false, false);
                        }

                        let verts = generate_wall(front, back, false);
                        mesh.add_vertices(verts, false, false);
                    }

                    // Generate the height difference
                    if front_sector.ceiling_height
                        != back_sector.ceiling_height
                    {
                        let front = front_sector.ceiling_height;
                        let back = back_sector.ceiling_height;
                        let verts = generate_wall(front, back, true);
                        mesh.add_vertices(verts, true, false);
                    }
                }
            }
        }

        index += 1;
        if index >= COLOR_TABLE.len() {
            index = 0;
        }
    }

    mesh
}

fn generate_sector_from_wad(
    map: &wad::Map,
    texture_queue: &mut TextureQueue,
    sector: &wad::Sector,
) -> Sector {
    let floor_mesh = generate_sector_floor(map, texture_queue, sector);
    let ceiling_mesh = generate_sector_ceiling(map, sector);
    let wall_mesh = generate_sector_wall(map, sector);

    Sector::new(floor_mesh, ceiling_mesh, wall_mesh)
}

fn generate_3d_map(
    wad: &wad::Wad,
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

    for sector in &map.sectors {
        let map_sector = generate_sector_from_wad(&map, texture_queue, sector);
        sectors.push(map_sector);
    }

    println!("Num Sectors: {}", sectors.len());

    Map::new(sectors)
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

fn read_texture_defs(wad: &Wad) -> Option<Vec<TextureDef>> {
    let mut texture_defs = Vec::new();

    if let Ok(index) = wad.find_dir("TEXTURE1") {
        let data = wad.read_dir(index).unwrap();

        let num_textures = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let num_textures = num_textures as usize;
        println!("Num Textures: {}", num_textures);

        let data_offset = 4;
        for i in 0..num_textures {
            let start = i * 4 + data_offset;

            let offset =
                u32::from_le_bytes(data[start..start + 4].try_into().unwrap());
            let offset = offset as usize;

            let name = &data[offset + 0..offset + 8];
            let null_pos =
                name.iter().position(|&c| c == 0).unwrap_or(name.len());
            let name = &name[..null_pos];
            let name = std::str::from_utf8(name).ok()?;
            let name = String::from(name);

            let masked = u32::from_le_bytes(
                data[offset + 8..offset + 12].try_into().ok()?,
            );

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
    }

    Some(texture_defs)
}

fn process_texture_defs(
    wad: &Wad,
    patch_names: &Vec<String>,
    texture_defs: &Vec<TextureDef>,
    color_map: &ColorMap,
    palette: &Palette,
) {
    for def in texture_defs {
        let mut pixels = vec![0u8; def.width * def.height * 4];
        for patch in &def.patches {
            let patch_name = &patch_names[patch.patch];

            let texture =
                read_patch_texture(wad, &patch_name, color_map, palette)
                    .expect("Failed to read patch texture");

            let mut xoff = patch.origin_x as isize;
            let mut yoff = patch.origin_y as isize;
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
        let path = format!("test/{}.png", def.name);
        write_texture_to_png(&path, &new_texture);
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
struct GltfPbr {
    base_color_factor: [f32; 4],
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
}

type BufferViewId = usize;
type MaterialId = usize;
type AccessorId = usize;
type SceneId = usize;
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
    scenes: Vec<GltfScene>,
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
            scenes: Vec::new(),
            meshes: Vec::new(),
            nodes: Vec::new(),
        }
    }

    fn create_material(&mut self, name: String, color: Vec4) -> MaterialId {
        let id = self.materials.len();
        let material = GltfMaterial {
            name,
            double_sided: false,
            pbr_metallic_roughness: GltfPbr {
                base_color_factor: [color.x, color.y, color.z, color.w],
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

    fn test(self) {
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

        use std::io::Write;
        let mut file = File::create("test.glb").unwrap();
        file.write_all(&bin_buffer).unwrap();
    }
}

fn write_gltf_file(map: Map) {
    let mut gltf = Gltf::new();

    let map_name = "E1M1";

    let scene_id = gltf.create_scene(map_name.to_string());

    for sector_index in 0..map.sectors.len() {
        let sector = &map.sectors[sector_index];

        let mesh_id = gltf.create_mesh(format!("Sector #{}", sector_index));

        let material_id = gltf.create_material(
            format!("Sector #{} Floor", sector_index),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        );

        gltf.add_mesh_primitive(mesh_id, &sector.floor_mesh, material_id);

        let material_id = gltf.create_material(
            format!("Sector #{} Ceiling", sector_index),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        );

        gltf.add_mesh_primitive(mesh_id, &sector.ceiling_mesh, material_id);

        let material_id = gltf.create_material(
            format!("Sector #{} Walls", sector_index),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
        );

        gltf.add_mesh_primitive(mesh_id, &sector.wall_mesh, material_id);

        let node_id =
            gltf.create_node(format!("Sector #{}-col", sector_index), mesh_id);

        gltf.add_node_to_scene(scene_id, node_id);
    }

    gltf.test();

    // let mut buffer: Vec<u8> = vec![];
    //
    // let (
    //     vertex_count,
    //     vertex_offset,
    //     vertex_length,
    //     index_count,
    //     index_offset,
    //     index_length,
    // ) = {
    //     let mesh = &sector.floor_mesh;
    //
    //     let vertex_byte_offset = buffer.len();
    //     let mut vertices = vec![];
    //     for vert in &mesh.vertex_buffer {
    //         vertices.push(vert.pos);
    //     }
    //
    //     for vertex in &vertices {
    //         let x = vertex.x / 50.0;
    //         let y = vertex.y / 50.0;
    //         let z = vertex.z / 50.0;
    //         buffer.extend_from_slice(&x.to_le_bytes());
    //         buffer.extend_from_slice(&y.to_le_bytes());
    //         buffer.extend_from_slice(&z.to_le_bytes());
    //     }
    //
    //     let vertex_byte_length = buffer.len();
    //
    //     let index_byte_offset = buffer.len();
    //
    //     let index_count = mesh.index_buffer.len();
    //     for index in &mesh.index_buffer {
    //         buffer.extend_from_slice(&index.to_le_bytes());
    //     }
    //
    //     let index_byte_length = index_count * 4;
    //
    //     (
    //         vertices.len(),
    //         vertex_byte_offset,
    //         vertex_byte_length,
    //         index_count,
    //         index_byte_offset,
    //         index_byte_length,
    //     )
    // };
    //
    // let (
    //     vertex_count2,
    //     vertex_offset2,
    //     vertex_length2,
    //     index_count2,
    //     index_offset2,
    //     index_length2,
    // ) = {
    //     let mesh = &sector.ceiling_mesh;
    //
    //     let vertex_byte_offset = buffer.len();
    //     let mut vertices = vec![];
    //     for vert in &mesh.vertex_buffer {
    //         vertices.push(vert.pos);
    //     }
    //
    //     for vertex in &vertices {
    //         let x = vertex.x / 50.0;
    //         let y = vertex.y / 50.0;
    //         let z = vertex.z / 50.0;
    //         buffer.extend_from_slice(&x.to_le_bytes());
    //         buffer.extend_from_slice(&y.to_le_bytes());
    //         buffer.extend_from_slice(&z.to_le_bytes());
    //     }
    //
    //     let vertex_byte_length = buffer.len();
    //
    //     let index_byte_offset = buffer.len();
    //
    //     let index_count = mesh.index_buffer.len();
    //     for index in &mesh.index_buffer {
    //         buffer.extend_from_slice(&index.to_le_bytes());
    //     }
    //
    //     let index_byte_length = index_count * 4;
    //
    //     (
    //         vertices.len(),
    //         vertex_byte_offset,
    //         vertex_byte_length,
    //         index_count,
    //         index_byte_offset,
    //         index_byte_length,
    //     )
    // };
    //
    // println!("Vertex: {} {}", vertex_offset, vertex_length);
    // println!("Index: {} {}", index_offset, index_length);
    //
    // println!("Vertex2: {} {}", vertex_offset2, vertex_length2);
    // println!("Index2: {} {}", index_offset2, index_length2);
    //
    // let test_buffer = GltfBuffer {
    //     byte_length: buffer.len(),
    // };
    //
    // let mut buffer_views = Vec::new();
    //
    // let vertex_buffer_view = GltfBufferView {
    //     buffer: 0,
    //     byte_length: vertex_length,
    //     byte_offset: vertex_offset,
    // };
    // buffer_views.push(vertex_buffer_view);
    //
    // let index_buffer_view = GltfBufferView {
    //     buffer: 0,
    //     byte_length: index_length,
    //     byte_offset: index_offset,
    // };
    // buffer_views.push(index_buffer_view);
    //
    // let vertex_buffer_view = GltfBufferView {
    //     buffer: 0,
    //     byte_length: vertex_length2,
    //     byte_offset: vertex_offset2,
    // };
    // buffer_views.push(vertex_buffer_view);
    //
    // let index_buffer_view = GltfBufferView {
    //     buffer: 0,
    //     byte_length: index_length2,
    //     byte_offset: index_offset2,
    // };
    // buffer_views.push(index_buffer_view);
    //
    // let mut accessors = vec![];
    // let position_accessor = GltfAccessor {
    //     buffer_view: 0,
    //     component_type: 5126,
    //     count: vertex_count,
    //     typ: "VEC3".to_string(),
    // };
    // let position_attrib = accessors.len();
    // accessors.push(position_accessor);
    //
    // let index_accessor = GltfAccessor {
    //     buffer_view: 1,
    //     component_type: 0x1405,
    //     count: index_count,
    //     typ: "SCALAR".to_string(),
    // };
    // let index_buffer = accessors.len();
    // accessors.push(index_accessor);
    //
    // let position_accessor = GltfAccessor {
    //     buffer_view: 2,
    //     component_type: 5126,
    //     count: vertex_count2,
    //     typ: "VEC3".to_string(),
    // };
    // let position_attrib2 = accessors.len();
    // accessors.push(position_accessor);
    //
    // let index_accessor = GltfAccessor {
    //     buffer_view: 3,
    //     component_type: 0x1405,
    //     count: index_count2,
    //     typ: "SCALAR".to_string(),
    // };
    // let index_buffer2 = accessors.len();
    // accessors.push(index_accessor);
    //
    // let test_material = GltfMaterial {
    //     name: "Test Material".to_string(),
    //     double_sided: false,
    //     pbr_metallic_roughness: GltfPbr {
    //         base_color_factor: [1.0, 0.0, 1.0, 1.0],
    //         metallic_factor: 1.0,
    //         roughness_factor: 1.0,
    //     },
    // };
    //
    // let test_material2 = GltfMaterial {
    //     name: "Test Material 2".to_string(),
    //     double_sided: false,
    //     pbr_metallic_roughness: GltfPbr {
    //         base_color_factor: [1.0, 0.0, 1.0, 1.0],
    //         metallic_factor: 1.0,
    //         roughness_factor: 1.0,
    //     },
    // };
    //
    // let mut test_mesh_attributes = HashMap::new();
    // test_mesh_attributes.insert("POSITION".to_string(), position_attrib);
    // // test_mesh_attributes.insert("NORMAL".to_string(), 0);
    // // test_mesh_attributes.insert("TEXCOORD_0".to_string(), 0);
    // //
    //
    // let test_mesh_primitive = GltfPrimitive {
    //     mode: 4,
    //     attributes: test_mesh_attributes,
    //     indices: index_buffer,
    //     material: 0,
    // };
    //
    // let mut test_mesh_attributes2 = HashMap::new();
    // test_mesh_attributes2.insert("POSITION".to_string(), position_attrib2);
    //
    // let test_mesh_primitive2 = GltfPrimitive {
    //     mode: 4,
    //     attributes: test_mesh_attributes2,
    //     indices: index_buffer2,
    //     material: 1,
    // };
    //
    // let test_mesh = GltfMesh {
    //     name: "Test Mesh".to_string(),
    //     primitives: vec![test_mesh_primitive, test_mesh_primitive2],
    // };
    //
    // let test_node = GltfNode {
    //     name: "Test Node".to_string(),
    //     mesh: 0,
    // };
    //
    // let scene = GltfScene {
    //     name: "Test Scene".to_string(),
    //     nodes: vec![0],
    // };
    //
    // let asset = GltfAsset {
    //     generator: "Testing".to_string(),
    //     version: "2.0".to_string(),
    // };
    //
    // let gltf_json = GltfJson {
    //     accessors,
    //     asset,
    //     buffer_views,
    //     buffers: vec![test_buffer],
    //     materials: vec![test_material, test_material2],
    //     meshes: vec![test_mesh],
    //     nodes: vec![test_node],
    //     scene: 0,
    //     scenes: vec![scene],
    // };
    //
    // let text = serde_json::to_string_pretty(&gltf_json).unwrap();
    // // NOTE(patrik): Debug JSON
    // // println!("{}", text);
}

fn main() {
    let args = Args::parse();
    println!("Args: {:?}", args);

    let output = if let Some(output) = args.output {
        PathBuf::from(output)
    } else {
        let mut path = PathBuf::from(args.wad_file.clone());
        path.set_extension("mup");
        path
    };

    // Read the raw wad file
    let data = read_file(args.wad_file);
    // Parse the wad
    let wad = Wad::parse(&data).expect("Failed to parse WAD file");

    std::fs::create_dir_all("test").expect("Failed to create 'test' folder");

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

    // FLOOR4_8 (flat)
    let texture =
        read_flat_texture(&wad, "FLOOR4_8", final_color_map, final_palette)
            .expect("Failed to read FLOOR4_8");
    let path = format!("test/FLOOR4_8.png");
    write_texture_to_png(&path, &texture);

    // TITLEPIC (PATCH FORMAT)
    let texture =
        read_patch_texture(&wad, "TITLEPIC", final_color_map, final_palette)
            .expect("Failed to read TITLEPIC");
    let path = format!("test/TITLEPIC.png");
    write_texture_to_png(&path, &texture);

    let patch_names =
        read_patch_names(&wad).expect("Failed to load patch names");
    // println!("Patch Names: {:#?}", patch_names);

    // assert!(!wad.find_dir("TEXTURE2").is_ok());
    let texture_defs =
        read_texture_defs(&wad).expect("Failed to read texture defs");
    // process_texture_defs(&wad, &patch_names, &texture_defs, final_color_map, final_palette);

    let map = if let Some(map) = args.map.as_ref() {
        map.as_str()
    } else {
        // TODO(patrik): If args.map is none then we should convert all
        // the maps
        "E1M1"
    };

    println!("Converting '{}' to mime map", map);

    let mut texture_queue = TextureQueue::new();

    let map = generate_3d_map(&wad, &mut texture_queue, map);
    write_gltf_file(map);

    // for t in texture_queue.textures {
    //     let texture = texture_loader
    //         .load(&wad, &t)
    //         .expect("Failed to load texture");
    //     println!("{}: {}, {}", t, texture.width, texture.height);
    // }
    //
    // let mut mime = mime::Mime::new();
    // mime.add_map(map);
    //
    // mime.save_to_file(output)
    //     .expect("Failed to save the generated map to the file");
}
