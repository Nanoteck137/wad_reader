use serde::{Serialize, Deserialize};
use crate::math::{Vec2, Vec3, Vec4};
use crate::Mesh;
use std::collections::HashMap;

type BufferViewId = usize;
type MaterialId = usize;
type AccessorId = usize;
type SamplerId = usize;
type TextureId = usize;
type SceneId = usize;
type ImageId = usize;
type MeshId = usize;
type NodeId = usize;

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
pub struct GltfTextureInfo {
    index: usize,
    tex_coord: usize,
}

impl GltfTextureInfo {
    pub fn new(texture_id: usize) -> Self {
        Self {
            index: texture_id,
            tex_coord: 0,
        }
    }
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

#[derive(Copy, Clone, PartialEq, Debug)]
enum DataTyp {
    Uint32,
    Vec2f,
    Vec3f,
    Vec4f,
}

pub struct Gltf {
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
    pub fn new() -> Self {
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

    pub fn create_sampler(&mut self, name: String) -> SamplerId {
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

    pub fn create_image(&mut self, name: String, data: &[u8]) -> ImageId {
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

    pub fn create_texture(
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

    pub fn create_material(
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

    pub fn create_mesh(&mut self, name: String) -> MeshId {
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

    pub fn add_mesh_primitive(
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

    pub fn create_node(&mut self, name: String, mesh_id: MeshId) -> NodeId {
        let id = self.nodes.len();
        let node = GltfNode {
            name,
            mesh: mesh_id,
        };

        self.nodes.push(node);
        id
    }

    pub fn create_scene(&mut self, name: String) -> SceneId {
        let id = self.scenes.len();
        let scene = GltfScene {
            name,
            nodes: Vec::new(),
        };

        self.scenes.push(scene);
        id
    }

    pub fn add_node_to_scene(&mut self, scene_id: SceneId, node_id: NodeId) {
        self.scenes[scene_id].nodes.push(node_id);
    }

    pub fn write_model(self) -> Vec<u8> {
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
