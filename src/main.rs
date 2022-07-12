use std::path::{ Path, PathBuf };
use std::fs::File;
use std::io::Read;
use std::io::BufWriter;

use clap::Parser;

use wad::Wad;

mod wad;
mod util;

/// TODO(patrik):
///   - Map format
///     - Textures

static COLOR_TABLE: [[f32; 4]; 10] = [
    [0.6705882352941176, 0.56078431372549020, 0.564705882352941200, 1.0],
    [0.7137254901960784, 0.53333333333333330, 0.223529411764705900, 1.0],
    [0.6705882352941176, 0.71372549019607840, 0.686274509803921600, 1.0],
    [0.9058823529411765, 0.55686274509803920, 0.725490196078431300, 1.0],
    [0.4823529411764706, 0.30196078431372547, 0.396078431372549000, 1.0],
    [0.4039215686274510, 0.88627450980392150, 0.027450980392156862, 1.0],
    [0.6745098039215687, 0.32941176470588235, 0.078431372549019600, 1.0],
    [0.9411764705882353, 0.68235294117647060, 0.843137254901960800, 1.0],
    [0.7176470588235294, 0.32941176470588235, 0.156862745098039200, 1.0],
    [0.6274509803921569, 0.31372549019607840, 0.011764705882352941, 1.0],
];

fn read_file<P>(path: P) -> Vec<u8>
    where P: AsRef<Path>
{
    let mut file = File::open(path).unwrap();

    let mut result = Vec::new();
    file.read_to_end(&mut result).unwrap();

    result
}

struct Mesh {
    vertex_buffer: Vec<mime::Vertex>,
    index_buffer: Vec<u32>,
}

impl Mesh {
    fn new() -> Self {
        Self {
            vertex_buffer: Vec::new(),
            index_buffer: Vec::new(),
        }
    }

    fn add_vertices(&mut self,
                    mut vertices: Vec<mime::Vertex>,
                    clockwise: bool,
                    cleanup: bool)
    {
        if cleanup {
            util::cleanup_lines(&mut vertices);
        }

        let triangles = util::triangulate(&vertices, clockwise);

        let index_offset = self.vertex_buffer.len();

        for v in &vertices {
            self.vertex_buffer.push(*v);
        }

        for i in &triangles {
            self.index_buffer.push(i + index_offset as u32);
        }
    }
}

fn generate_sector_floor(map: &wad::Map, sector: &wad::Sector) -> mime::Mesh {
    let mut mesh = Mesh::new();

    let mut index = 0;

    for sub_sector in &sector.sub_sectors {
        let mut vertices = Vec::new();

        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];
            let start = map.vertex(segment.start_vertex);

            let pos = [start.x, sector.floor_height, start.y];
            let uv = [start.x, start.y];
            let color = COLOR_TABLE[index];
            vertices.push(mime::Vertex::new(pos, uv, color));
        }

        index += 1;
        if index >= COLOR_TABLE.len() {
            index = 0;
        }

        mesh.add_vertices(vertices, true, true);
    }

    mime::Mesh::new(mesh.vertex_buffer, mesh.index_buffer)
}

fn generate_sector_ceiling(map: &wad::Map, sector: &wad::Sector)
    -> mime::Mesh
{
    let mut mesh = Mesh::new();

    let mut index = 0;

    for sub_sector in &sector.sub_sectors {
        let mut vertices = Vec::new();

        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];
            let start = map.vertex(segment.start_vertex);

            let pos = [start.x, sector.ceiling_height, start.y];
            let uv = [start.x, start.y];
            let color = COLOR_TABLE[index];
            vertices.push(mime::Vertex::new(pos, uv, color));
        }

        index += 1;
        if index >= COLOR_TABLE.len() {
            index = 0;
        }

        mesh.add_vertices(vertices, false, true);
    }

    mime::Mesh::new(mesh.vertex_buffer, mesh.index_buffer)
}

fn generate_sector_wall(map: &wad::Map, sector: &wad::Sector) -> mime::Mesh {
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

                if linedef.flags & wad::LINEDEF_FLAG_IMPASSABLE == wad::LINEDEF_FLAG_IMPASSABLE &&
                    linedef.flags & wad::LINEDEF_FLAG_TWO_SIDED != wad::LINEDEF_FLAG_TWO_SIDED
                {
                    let uv = [0.0, 0.0];

                    let pos = [start.x, sector.floor_height, start.y];
                    wall.push(mime::Vertex::new(pos, uv, color));

                    let pos = [end.x, sector.floor_height, end.y];
                    wall.push(mime::Vertex::new(pos, uv, color));

                    let pos = [end.x, sector.ceiling_height, end.y];
                    wall.push(mime::Vertex::new(pos, uv, color));

                    let pos = [start.x, sector.ceiling_height, start.y];
                    wall.push(mime::Vertex::new(pos, uv, color));
                }

                mesh.add_vertices(wall, false, false);

                let mut generate_wall = |front, back, clockwise| {
                    let mut verts = Vec::new();

                    let color = COLOR_TABLE[index];
                    let uv = [0.0, 0.0];

                    let pos = [start.x, front, start.y];
                    verts.push(mime::Vertex::new(pos, uv, color));

                    let pos = [end.x, front, end.y];
                    verts.push(mime::Vertex::new(pos, uv, color));

                    let pos = [end.x, back, end.y];
                    verts.push(mime::Vertex::new(pos, uv, color));

                    let pos = [start.x, back, start.y];
                    verts.push(mime::Vertex::new(pos, uv, color));

                    mesh.add_vertices(verts, clockwise, false);

                    index += 1;
                    if index >= COLOR_TABLE.len() {
                        index = 0;
                    }
                };

                if linedef.front_sidedef.is_some() &&
                    linedef.back_sidedef.is_some()
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

                        generate_wall(front, back, false);
                    }

                    // Generate the height difference
                    if front_sector.ceiling_height != back_sector.ceiling_height {
                        let front = front_sector.ceiling_height;
                        let back = back_sector.ceiling_height;
                        generate_wall(front, back, true);
                    }
                }
            }
        }

        index += 1;
        if index >= COLOR_TABLE.len() {
            index = 0;
        }
    }

    mime::Mesh::new(mesh.vertex_buffer, mesh.index_buffer)
}

fn generate_sector_from_wad(map: &wad::Map, sector: &wad::Sector)
    -> mime::Sector
{
    let floor_mesh = generate_sector_floor(map, sector);
    let ceiling_mesh = generate_sector_ceiling(map, sector);
    let wall_mesh = generate_sector_wall(map, sector);

    mime::Sector::new(floor_mesh, ceiling_mesh, wall_mesh)
}

fn generate_3d_map(wad: &wad::Wad, map_name: &str) -> mime::Map {
    // Construct an map with map from the wad
    let map = wad::Map::parse_from_wad(&wad, map_name)
        .expect("Failed to load wad map");

    let mut sectors = Vec::new();

    /*
    let map_sector = generate_sector_from_wad(&map, &map.sectors[50]);
    sectors.push(map_sector);
    */

    for sector in &map.sectors {
        let map_sector = generate_sector_from_wad(&map, sector);
        sectors.push(map_sector);
    }

    println!("Num Sectors: {}", sectors.len());

    mime::Map::new(sectors)
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
    b: u8
}

const MAX_PALETTE_COLORS: usize = 256;
const MAX_COLOR_MAPS: usize = 34;

const FLAT_TEXTURE_WIDTH: usize = 64;
const FLAT_TEXTURE_HEIGHT: usize = 64;

struct Palette {
    colors: [PaletteColor; MAX_PALETTE_COLORS],
}

impl Palette {
    fn get(&self, index: usize) -> PaletteColor {
        self.colors[index]
    }
}

struct ColorMap {
    map: [usize; MAX_PALETTE_COLORS]
}

impl ColorMap {
    fn get(&self, index: usize) -> usize {
        self.map[index]
    }

    fn get_color_from_palette(&self, palette: &Palette, index: usize)
        -> PaletteColor
    {
        let palette_index = self.get(index);
        palette.get(palette_index)
    }
}

fn read_all_palettes(wad: &Wad) -> Option<Vec<Palette>> {
    if let Ok(index) = wad.find_dir("PLAYPAL") {
        let playpal = wad.read_dir(index)
            .expect("Failed to get PLAYPAL data");
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
        let color_map_table = wad.read_dir(index)
            .expect("Failed to get COLORMAP data");

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
    pixels: Vec<u8>,
}

fn read_flat_texture(wad: &Wad, name: &str,
                     color_map: &ColorMap, palette: &Palette)
    -> Option<Texture>
{
    if let Ok(index) = wad.find_dir(name) {

        let texture_data = wad.read_dir(index).ok()?;

        let mut pixels = vec![0u8; FLAT_TEXTURE_WIDTH * FLAT_TEXTURE_HEIGHT * 4];

        for x in 0..FLAT_TEXTURE_WIDTH {
            for y in 0..FLAT_TEXTURE_HEIGHT {
                let start = x + y * FLAT_TEXTURE_WIDTH;
                let index = texture_data[start];
                let index = index as usize;

                let color =
                    color_map.get_color_from_palette(palette, index);

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
            pixels,
        });
    }

    None
}

fn read_patch_texture(wad: &Wad, name: &str,
                      color_map: &ColorMap, palette: &Palette)
    -> Option<Texture>
{
    if let Ok(index) = wad.find_dir(name) {
        let texture_data = wad.read_dir(index).ok()?;

        let width = u16::from_le_bytes(texture_data[0..2].try_into().unwrap());
        let height = u16::from_le_bytes(texture_data[2..4].try_into().unwrap());

        // TODO(patrik): Should we use these
        let left_offset = i16::from_le_bytes(texture_data[4..6].try_into().unwrap());
        let top_offset = i16::from_le_bytes(texture_data[6..8].try_into().unwrap());

        assert!(left_offset == 0 && top_offset == 0);

        let width = width as usize;
        let height = height as usize;

        let mut pixels = vec![0u8; width * height * 4];

        let start_offset = 8;
        for x in 0..width {
            let start = x * 4 + start_offset;
            let offset = u32::from_le_bytes(texture_data[start..start + 4].try_into().unwrap());
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
            pixels,
        });
    }

    None
}

fn write_texture_to_png<P>(path: P, texture: &Texture)
    where P: AsRef<Path>
{
    let file = File::create(path).unwrap();
    let ref mut file_writer = BufWriter::new(file);

    let mut encoder = png::Encoder::new(file_writer,
                                        texture.width as u32,
                                        texture.height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&texture.pixels).unwrap();
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
    let wad = Wad::parse(&data)
        .expect("Failed to parse WAD file");

    std::fs::create_dir_all("test")
        .expect("Failed to create 'test' folder");

    let palettes = read_all_palettes(&wad)
        .expect("Failed to read palettes");
    let final_palette = &palettes[0];

    let color_maps = read_all_color_maps(&wad)
        .expect("Failed to read color maps");
    let final_color_map = &color_maps[0];

    // FLOOR4_8 (flat)
    let texture = read_flat_texture(&wad, "FLOOR4_8", final_color_map, final_palette)
        .expect("Failed to read FLOOR4_8");
    let path = format!("test/FLOOR4_8.png");
    write_texture_to_png(&path, &texture);

    // TITLEPIC (PATCH FORMAT)
    let texture = read_patch_texture(&wad, "TITLEPIC", final_color_map, final_palette)
        .expect("Failed to read TITLEPIC");
    let path = format!("test/TITLEPIC.png");
    write_texture_to_png(&path, &texture);

    let map = if let Some(map) = args.map.as_ref() {
        map.as_str()
    } else {
        // TODO(patrik): If args.map is none then we should convert all
        // the maps
        "E1M1"
    };

    println!("Converting '{}' to mime map", map);

    let map = generate_3d_map(&wad, map);
    map.save_to_file(output)
        .expect("Failed to save the generated map to the file");
}
