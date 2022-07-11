use std::path::{ Path, PathBuf };
use std::fs::File;
use std::io::Read;
use std::io::BufWriter;

use clap::Parser;

use wad::Wad;

mod wad;
mod util;

/// TODO(patrik):
///   - Parse texture data
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

            let color = COLOR_TABLE[index];
            vertices.push(mime::Vertex::new(start.x, sector.floor_height, start.y, color));
        }

        index += 1;
        if index >= COLOR_TABLE.len() {
            index = 0;
        }

        mesh.add_vertices(vertices, true, true);
    }

    mime::Mesh::new(mesh.vertex_buffer, mesh.index_buffer)
}

fn generate_sector_ceiling(map: &wad::Map, sector: &wad::Sector) -> mime::Mesh {
    let mut mesh = Mesh::new();

    let mut index = 0;

    for sub_sector in &sector.sub_sectors {
        let mut vertices = Vec::new();

        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];
            let start = map.vertex(segment.start_vertex);

            let color = COLOR_TABLE[index];
            vertices.push(mime::Vertex::new(start.x, sector.ceiling_height, start.y, color));
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
                    wall.push(mime::Vertex::new(start.x, sector.floor_height, start.y, color));
                    wall.push(mime::Vertex::new(end.x, sector.floor_height, end.y, color));
                    wall.push(mime::Vertex::new(end.x, sector.ceiling_height, end.y, color));
                    wall.push(mime::Vertex::new(start.x, sector.ceiling_height, start.y, color));
                }

                mesh.add_vertices(wall, false, false);

                let mut generate_wall = |front, back, clockwise| {
                    let mut verts = Vec::new();

                    let color = COLOR_TABLE[index];
                    verts.push(
                        mime::Vertex::new(start.x, front, start.y, color));
                    verts.push(
                        mime::Vertex::new(end.x, front, end.y, color));
                    verts.push(
                        mime::Vertex::new(end.x, back, end.y, color));
                    verts.push(
                        mime::Vertex::new(start.x, back, start.y, color));

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

/*
fn generate_sector_floor_ceiling(map: &wad::Map, sector: &wad::Sector)
    -> mime::Mesh
{
    let mut index = 0;

    for sub_sector in &sector.sub_sectors {
        let mut floor = Vec::new();
        let mut ceiling = Vec::new();

        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];
            let start = map.vertex(segment.start_vertex);

            let color = COLOR_TABLE[index];
            floor.push(mime::Vertex::new(start.x, sector.floor_height, start.y, color));
            ceiling.push(mime::Vertex::new(start.x, sector.ceiling_height, start.y, color));
        }

        index += 1;
        if index >= COLOR_TABLE.len() {
            index = 0;
        }

        mesh.add_vertices(floor, true, true);
        mesh.add_vertices(ceiling, false, true);
    }
}
*/

/*
fn generate_sector_walls(map: &wad::Map, sector: &wad::Sector,
                         mesh: &mut Mesh)
{
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
                    wall.push(mime::Vertex::new(start.x, sector.floor_height, start.y, color));
                    wall.push(mime::Vertex::new(end.x, sector.floor_height, end.y, color));
                    wall.push(mime::Vertex::new(end.x, sector.ceiling_height, end.y, color));
                    wall.push(mime::Vertex::new(start.x, sector.ceiling_height, start.y, color));
                }

                mesh.add_vertices(wall, false, false);
            }
        }

        index += 1;
        if index >= COLOR_TABLE.len() {
            index = 0;
        }
    }
}
*/

/*
fn generate_sector_extra(map: &wad::Map, sector: &wad::Sector,
                         mesh: &mut Mesh)
{
    let mut index = 0;

    for sub_sector in &sector.sub_sectors {
        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];

            if segment.linedef != 0xffff {
                let linedef = map.linedefs[segment.linedef];
                let line = linedef.line;

                let start = map.vertex(line.start_vertex);
                let end = map.vertex(line.end_vertex);

                let mut generate_wall = |front, back, clockwise| {
                    let mut verts = Vec::new();

                    let color = COLOR_TABLE[index];
                    verts.push(
                        mime::Vertex::new(start.x, front, start.y, color));
                    verts.push(
                        mime::Vertex::new(end.x, front, end.y, color));
                    verts.push(
                        mime::Vertex::new(end.x, back, end.y, color));
                    verts.push(
                        mime::Vertex::new(start.x, back, start.y, color));

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
    }
}
*/

fn generate_sector_from_wad(map: &wad::Map,
                            sector: &wad::Sector)
    -> mime::Sector
{
    let mut mesh = Mesh::new();

    // let floor_mesh = generate_sector_floor_ceiling(map, sector);
    // generate_sector_walls(map, sector, &mut mesh);
    // generate_sector_extra(map, sector, &mut mesh);
    //
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

    let mut final_palette = None;
    if let Ok(index) = wad.find_dir("PLAYPAL") {
        let playpal = wad.read_dir(index)
            .expect("Failed to get PLAYPAL data");
        // One palette entry (R, G, B) 3 bytes
        let num_colors = playpal.len() / 3;
        // 256 palette entries per palette
        let palette_count = num_colors / 256;
        println!("Palette Count: {}", palette_count);

        std::fs::create_dir_all("test")
            .expect("Failed to create 'test' folder");

        for palette in 0..palette_count {
            println!("Writing palette #{}", palette);
            let mut colors = [PaletteColor::default(); 256];

            let data_start = palette * (256 * 3);
            for color_index in 0..256 {
                let start = color_index * 3 + data_start;
                let r = playpal[start + 0];
                let g = playpal[start + 1];
                let b = playpal[start + 2];
                colors[color_index] = PaletteColor { r, g, b };
                // println!("Byte Offset: {}, ({}, {}, {})", data_start, r, g, b);
            }

            let path = format!("test/{}_pal.png", palette);
            let path = Path::new(&path);
            let file = File::create(path).unwrap();
            let ref mut w = BufWriter::new(file);
            let mut encoder = png::Encoder::new(w, 16, 16);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();

            let mut pixels = Vec::new();
            for x in 0..16 {
                for y in 0..16 {
                    let index = x + y * 16;
                    let color = colors[index];
                    pixels.push(color.r);
                    pixels.push(color.g);
                    pixels.push(color.b);
                    pixels.push(0xffu8);
                }
            }

            writer.write_image_data(&pixels).unwrap();

            if palette == 0 {
                final_palette = Some(colors);
            }
        }
    }

    let mut final_color_maps = None;
    if let Ok(index) = wad.find_dir("COLORMAP") {
        let color_map_table = wad.read_dir(index)
            .expect("Failed to get COLORMAP data");

        let num_color_maps = 34;

        let mut color_maps = [[0usize; 256]; 34];

        let mut pixels = Vec::new();
        for color_map_index in 0..num_color_maps {
            let data_start = color_map_index * 256;
            let mut color_map = [0usize; 256];
            for index in 0..256 {
                let start = index + data_start;
                let palette_index = color_map_table[start] as usize;
                color_map[index] = palette_index;

                let color = final_palette.unwrap()[palette_index];
                if color_map_index == 0 {
                    pixels.push(0xffu8);
                    pixels.push(0x00u8);
                    pixels.push(0xffu8);
                    pixels.push(0xffu8);
                } else {
                    pixels.push(color.r);
                    pixels.push(color.g);
                    pixels.push(color.b);
                    pixels.push(0xffu8);
                }
            }

            color_maps[color_map_index] = color_map;
        }

        let path = format!("test/colormap.png");
        let path = Path::new(&path);
        let file = File::create(path).unwrap();
        let ref mut w = BufWriter::new(file);
        let mut encoder = png::Encoder::new(w, 256, 34);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();

        final_color_maps = Some(color_maps);
    }

    // FLOOR4_8 (flat) NEEDS TO BE ROTATED
    if let Ok(index) = wad.find_dir("FLOOR4_8") {
        const TEXTURE_WIDTH: usize = 64;
        const TEXTURE_HEIGHT: usize = 64;

        let texture_data = wad.read_dir(index)
            .expect("Failed to get FLOOR4_8 data");

        let mut pixels = [0u8; TEXTURE_WIDTH * TEXTURE_HEIGHT * 4];

        for x in 0..TEXTURE_WIDTH {
            for y in 0..TEXTURE_HEIGHT {
                let start = x + y * TEXTURE_WIDTH;
                let index = texture_data[start];

                let color_map = final_color_maps.unwrap()[0];
                let palette_index = color_map[index as usize];

                let color = final_palette.unwrap()[palette_index];

                let img_index = x + y * TEXTURE_WIDTH;
                pixels[img_index * 4 + 0] = color.r;
                pixels[img_index * 4 + 1] = color.g;
                pixels[img_index * 4 + 2] = color.b;
                pixels[img_index * 4 + 3] = 0xffu8;
            }
        }

        let path = format!("test/FLOOR4_8.png");
        let path = Path::new(&path);
        let file = File::create(path).unwrap();
        let ref mut w = BufWriter::new(file);
        let mut encoder = png::Encoder::new(w, TEXTURE_WIDTH as u32, TEXTURE_HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
    }

    // TITLEPIC (PATCH FORMAT)
    if let Ok(index) = wad.find_dir("TITLEPIC") {
        let texture_data = wad.read_dir(index)
            .expect("Failed to get TITLEPIC data");

        let width = u16::from_le_bytes(texture_data[0..2].try_into().unwrap());
        let height = u16::from_le_bytes(texture_data[2..4].try_into().unwrap());
        let left_offset = i16::from_le_bytes(texture_data[4..6].try_into().unwrap());
        let top_offset = i16::from_le_bytes(texture_data[6..8].try_into().unwrap());
        println!("Width: {} Height: {}", width, height);
        println!("Left: {} Top: {}", left_offset, top_offset);
        println!("Texture Data Length: {:#x}", texture_data.len());

        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let color_map = final_color_maps.unwrap()[0];

        for x in 0..(width as usize) {
            let start = 8 + x * 4;
            let offset = u32::from_le_bytes(texture_data[start..start + 4].try_into().unwrap());
            let offset = offset as usize;
            // println!("Offset: {:#x}", offset);

            let mut new_offset = offset;
            let mut y_offset = 0;
            loop {
                let topdelta = texture_data[new_offset];
                if topdelta == 0xff {
                    break;
                }

                let length = texture_data[new_offset + 1];
                println!("{:#x}: Top: {} Length: {}", new_offset, topdelta, length);

                let start = new_offset + 2;
                for data_offset in 0..(length as usize) {
                    let index = texture_data[start + data_offset];
                    let palette_index = color_map[index as usize];

                    let color = final_palette.unwrap()[palette_index];

                    let y = y_offset;
                    let img_index = x + y * width as usize;
                    pixels[img_index * 4 + 0] = color.r;
                    pixels[img_index * 4 + 1] = color.g;
                    pixels[img_index * 4 + 2] = color.b;
                    pixels[img_index * 4 + 3] = 0xffu8;

                    y_offset += 1;
                }

                new_offset += length as usize + 4;
            }
        }

        /*
        for x in 0..TEXTURE_WIDTH {
            for y in 0..TEXTURE_HEIGHT {
                let start = x + y * TEXTURE_WIDTH;
                let index = texture_data[start];

                let color_map = final_color_maps.unwrap()[0];
                let palette_index = color_map[index as usize];

                let color = final_palette.unwrap()[palette_index];

                let img_index = x + y * TEXTURE_WIDTH;
                pixels[img_index * 4 + 0] = color.r;
                pixels[img_index * 4 + 1] = color.g;
                pixels[img_index * 4 + 2] = color.b;
                pixels[img_index * 4 + 3] = 0xffu8;
            }
        }
        */

        let path = format!("test/TITLEPIC.png");
        let path = Path::new(&path);
        let file = File::create(path).unwrap();
        let ref mut w = BufWriter::new(file);
        let mut encoder = png::Encoder::new(w, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
    }

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
