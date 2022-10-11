use std::path::{Path, PathBuf};
use std::collections::HashMap;

use clap::Parser;

use wad::Wad;
use math::{Vec2, Vec3, Vec4};
use texture::{TextureLoader, TextureQueue, Texture};
use gltf::{Gltf, GltfTextureInfo};
use polygon::{Quad, Mesh, Vertex};

mod gen;
mod gltf;
mod math;
mod polygon;
mod texture;
mod util;
mod wad;

/// TODO(patrik):
///   - Lazy loading textures
///   - Debug Dumping Textures
///   - Add Debug Flags
///     - View Slopes
///     - View Normals
///     - View UVs

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

    fn gen_sector(
        context: &mut gen::Context,
        wad_map: &wad::Map,
        wad_sector: &wad::Sector,
    ) -> Self {
        let floor_mesh = gen::gen_floor(context, wad_map, wad_sector);

        let ceiling_mesh = gen::gen_ceiling(context, wad_map, wad_sector);

        let (wall_quads, slope_quads) =
            gen::gen_walls(context, wad_map, wad_sector);

        Sector::new(floor_mesh, ceiling_mesh, wall_quads, slope_quads)
    }
}

struct Map {
    sectors: Vec<Sector>,
}

impl Map {
    fn new(sectors: Vec<Sector>) -> Self {
        Self { sectors }
    }

    fn gen_map(context: &mut gen::Context, wad_map: &wad::Map) -> Self {
        let mut sectors = Vec::new();

        for wad_sector in &wad_map.sectors {
            let map_sector = Sector::gen_sector(context, &wad_map, wad_sector);

            sectors.push(map_sector);
        }

        Map::new(sectors)
    }
}

fn write_map_gltf<P>(context: &gen::Context, map: Map, output_file: P)
where
    P: AsRef<Path>,
{
    let mut gltf = Gltf::new();

    let map_name = "E1M1";

    let scene_id = gltf.create_scene(map_name.to_string());
    let texture_sampler = gltf.create_sampler("Default Sampler".to_string());

    let mut textures = Vec::new();
    for t in &context.texture_queue.textures {
        if let Some(texture) = context.texture_loader.load(&t) {
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
                // None,
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

    let texture_queue = TextureQueue::new();

    // Construct an map with map from the wad
    let wad_map =
        wad::Map::parse_from_wad(&wad, map).expect("Failed to load wad map");

    let mut context = gen::Context::new(texture_loader, texture_queue);

    let map = Map::gen_map(&mut context, &wad_map);
    write_map_gltf(&context, map, output);
}
