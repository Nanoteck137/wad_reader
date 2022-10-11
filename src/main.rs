use std::path::{Path, PathBuf};
use std::collections::HashMap;

use clap::Parser;

use wad::Wad;
use math::{Vec2, Vec3, Vec4};
use texture::{TextureLoader, TextureQueue, Texture};
use gltf::{Gltf, GltfTextureInfo};

mod gltf;
mod math;
mod texture;
mod util;
mod wad;

/// TODO(patrik):
///   - Lazy loading textures

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
pub struct Vertex {
    pub pos: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub color: Vec4,
}

impl Vertex {
    pub fn new(pos: Vec3, normal: Vec3, uv: Vec2, color: Vec4) -> Self {
        Self {
            pos,
            normal,
            uv,
            color,
        }
    }
}

pub struct Mesh {
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
}

impl Map {
    fn new(sectors: Vec<Sector>) -> Self {
        Self { sectors }
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

    let w = 1.0 / texture.width() as f32;
    let h = 1.0 / texture.height() as f32;

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

    let w = 1.0 / texture.width() as f32;
    let h = 1.0 / texture.height() as f32;

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

fn generate_sector_wall(
    map: &wad::Map,
    texture_loader: &TextureLoader,
    texture_queue: &mut TextureQueue,
    sector: &wad::Sector,
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
    map: &wad::Map,
    texture_loader: &TextureLoader,
    texture_queue: &mut TextureQueue,
    sector: &wad::Sector,
) -> Sector {
    let floor_mesh =
        generate_sector_floor(map, texture_loader, texture_queue, sector);
    let ceiling_mesh =
        generate_sector_ceiling(map, texture_loader, texture_queue, sector);

    let (wall_quads, slope_quads) =
        generate_sector_wall(map, texture_loader, texture_queue, sector);

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

    for sector in &map.sectors {
        let map_sector = generate_sector_from_wad(
            &map,
            texture_loader,
            texture_queue,
            sector,
        );
        sectors.push(map_sector);
    }

    println!("Num Sectors: {}", sectors.len());

    Map::new(sectors)
}

fn write_map_gltf<P>(
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
            let png = util::write_texture_to_png(texture);
            let image_id = gltf.create_image(t.clone(), &png);
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
            Some(GltfTextureInfo::new(
                textures[sector.floor_mesh.texture_id.unwrap_or(0)],
            )),
        );

        gltf.add_mesh_primitive(mesh_id, &sector.floor_mesh, material_id);

        let material_id = gltf.create_material(
            format!("Sector #{} Ceiling", sector_index),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            Some(GltfTextureInfo::new(
                textures[sector.ceiling_mesh.texture_id.unwrap_or(0)],
            )),
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
                Some(GltfTextureInfo::new(textures[texture_id])),
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

        let material_id = gltf.create_material(
            format!("Sector #{}: Slope Mesh", sector_index),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            None,
        );

        gltf.add_mesh_primitive(slope_mesh_id, &slope_mesh, material_id);

        let extra_node_id = gltf.create_node(
            format!("Sector #{}: Slope Mesh", sector_index),
            slope_mesh_id,
        );
        gltf.add_node_to_scene(scene_id, extra_node_id);
    }

    let data = gltf.write_model();
    util::write_binary_file(output_file, &data);
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

    let palettes =
        texture::read_all_palettes(&wad).expect("Failed to read palettes");
    let final_palette = &palettes[0];

    let color_maps =
        texture::read_all_color_maps(&wad).expect("Failed to read color maps");
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
    write_map_gltf(map, &texture_queue, &texture_loader, output);
}
