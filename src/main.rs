use std::path::Path;
use std::fs::File;
use std::io::Read;

use wad::Wad;

mod wad;
mod util;

/// TODO(patrik):
///   - Parse texture data
///   - Map format
///     - Seperate vertex and index buffers for the different sectors
///       of the map
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

fn generate_sector_floor_ceiling(map: &wad::Map, sector: &wad::Sector,
                                 mesh: &mut Mesh)
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

                if linedef.front_sidedef.is_some() && linedef.back_sidedef.is_some() {
                    let front_sidedef = linedef.front_sidedef.unwrap();
                    let front_sidedef = map.sidedefs[front_sidedef];

                    let back_sidedef = linedef.back_sidedef.unwrap();
                    let back_sidedef = map.sidedefs[back_sidedef];

                    let front_sector = &map.sectors[front_sidedef.sector];
                    let back_sector = &map.sectors[back_sidedef.sector];

                    // Generate the floor difference
                    if front_sector.floor_height != back_sector.floor_height {
                        // TODO(patrik): Generate the vertices

                        let front = front_sector.floor_height;
                        let back = back_sector.floor_height;

                        let mut verts = Vec::new();

                        let color = COLOR_TABLE[index]; //[1.0, 0.0, 1.0, 1.0];
                        verts.push(mime::Vertex::new(start.x, front, start.y, color));
                        verts.push(mime::Vertex::new(end.x, front, end.y, color));
                        verts.push(mime::Vertex::new(end.x, back, end.y, color));
                        verts.push(mime::Vertex::new(start.x, back, start.y, color));

                        mesh.add_vertices(verts, false, false);

                        index += 1;
                        if index >= COLOR_TABLE.len() {
                            index = 0;
                        }
                    }

                    // Generate the height difference
                    if front_sector.ceiling_height != back_sector.ceiling_height {
                        // TODO(patrik): Generate the vertices

                        let front = front_sector.ceiling_height;
                        let back = back_sector.ceiling_height;

                        let mut verts = Vec::new();

                        let color = COLOR_TABLE[index]; // [1.0, 0.0, 1.0, 1.0];
                        verts.push(mime::Vertex::new(start.x, front, start.y, color));
                        verts.push(mime::Vertex::new(end.x, front, end.y, color));
                        verts.push(mime::Vertex::new(end.x, back, end.y, color));
                        verts.push(mime::Vertex::new(start.x, back, start.y, color));

                        mesh.add_vertices(verts, true, false);

                        index += 1;
                        if index >= COLOR_TABLE.len() {
                            index = 0;
                        }
                    }
                }
            }
        }
    }
}

fn generate_sector_from_wad(map: &wad::Map,
                            sector: &wad::Sector)
    -> mime::Sector
{
    let mut mesh = Mesh::new();

    generate_sector_floor_ceiling(map, sector, &mut mesh);
    generate_sector_walls(map, sector, &mut mesh);
    generate_sector_extra(map, sector, &mut mesh);

    mime::Sector::new(mesh.vertex_buffer, mesh.index_buffer)
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

    mime::Map::new(sectors)
}

fn main() {
    // Read the raw wad file
    let data = read_file("doom1.wad");
    // Parse the wad
    let wad = Wad::parse(&data)
        .expect("Failed to parse WAD file");

    let map = generate_3d_map(&wad, "E1M1");
    map.save_to_file("map.mup")
        .expect("Failed to save the generated map to the file");
}
