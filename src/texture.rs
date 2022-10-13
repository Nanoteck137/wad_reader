use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::wad::Wad;
use crate::util;

const MAX_PALETTE_COLORS: usize = 256;
const MAX_COLOR_MAPS: usize = 34;

const FLAT_TEXTURE_WIDTH: usize = 64;
const FLAT_TEXTURE_HEIGHT: usize = 64;

struct Patch {
    name: String,
    origin_x: isize,
    origin_y: isize,
}

struct TextureComposition {
    patches: Vec<Patch>,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TextureTyp {
    Flat,
    Patch,
    Texture,
}

pub struct Texture {
    typ: TextureTyp,
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    composition: Option<TextureComposition>,
}

impl Texture {
    pub fn new(
        typ: TextureTyp,
        width: usize,
        height: usize,
        pixels: Vec<u8>,
    ) -> Self {
        Self {
            typ,
            width,
            height,
            pixels,
            composition: None,
        }
    }

    pub fn typ(&self) -> TextureTyp {
        self.typ
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct PaletteColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Clone)]
pub struct Palette {
    colors: [PaletteColor; MAX_PALETTE_COLORS],
}

impl Palette {
    pub fn get(&self, index: usize) -> PaletteColor {
        self.colors[index]
    }
}

#[derive(Clone)]
pub struct ColorMap {
    map: [usize; MAX_PALETTE_COLORS],
}

impl ColorMap {
    pub fn get(&self, index: usize) -> usize {
        self.map[index]
    }

    pub fn get_color_from_palette(
        &self,
        palette: &Palette,
        index: usize,
    ) -> PaletteColor {
        let palette_index = self.get(index);
        palette.get(palette_index)
    }
}

pub fn read_all_palettes(wad: &Wad) -> Option<Vec<Palette>> {
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

pub fn read_all_color_maps(wad: &Wad) -> Option<Vec<ColorMap>> {
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

pub fn read_flat_texture(
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

        return Some(Texture::new(
            TextureTyp::Flat,
            FLAT_TEXTURE_WIDTH,
            FLAT_TEXTURE_HEIGHT,
            pixels,
        ));
    }

    None
}

pub fn read_patch_texture(
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

        let _left_offset =
            i16::from_le_bytes(texture_data[4..6].try_into().unwrap());
        let _top_offset =
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

        return Some(Texture::new(TextureTyp::Patch, width, height, pixels));
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
    wad: &Wad,
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
    texture_loader: &TextureLoader,
    patch_names: &Vec<String>,
    texture_defs: &Vec<TextureDef>,
) -> HashMap<String, Texture> {
    let mut result = HashMap::new();

    for def in texture_defs {
        let mut patches = Vec::new();
        let mut pixels = vec![0u8; def.width * def.height * 4];

        for patch in &def.patches {
            let patch_name = &patch_names[patch.patch];

            let (_patch_texture_id, patch_texture) = texture_loader
                .load_from_name(&patch_name)
                .expect("Failed to read patch texture");

            let patch_def = Patch {
                name: patch_name.clone(),
                origin_x: patch.origin_x as isize,
                origin_y: patch.origin_y as isize,
            };
            patches.push(patch_def);

            let xoff = patch.origin_x as isize;
            let yoff = patch.origin_y as isize;
            for sy in 0..patch_texture.height() {
                for sx in 0..patch_texture.width() {
                    let source_index = sx + sy * patch_texture.width();

                    let x = sx as isize + xoff;
                    let y = sy as isize + yoff;

                    if x < 0 || y < 0 {
                        continue;
                    }

                    if x >= def.width as isize || y >= def.height as isize {
                        continue;
                    }

                    let dest_index = (x as usize) + (y as usize) * def.width;

                    let texture_pixels = patch_texture.pixels();
                    pixels[dest_index * 4 + 0] =
                        texture_pixels[source_index * 4 + 0];
                    pixels[dest_index * 4 + 1] =
                        texture_pixels[source_index * 4 + 1];
                    pixels[dest_index * 4 + 2] =
                        texture_pixels[source_index * 4 + 2];
                    pixels[dest_index * 4 + 3] =
                        texture_pixels[source_index * 4 + 3];
                }
            }
        }

        let composition = TextureComposition { patches };
        let mut new_texture =
            Texture::new(TextureTyp::Texture, def.width, def.height, pixels);
        new_texture.composition = Some(composition);
        result.insert(def.name.clone(), new_texture);
    }

    result
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

pub struct TextureLoader {
    color_map: ColorMap,
    palette: Palette,

    missing_texture_id: usize,
    textures: Vec<(String, Texture)>,
}

impl TextureLoader {
    pub fn new(
        wad: &Wad,
        color_map: ColorMap,
        palette: Palette,
    ) -> Option<Self> {
        assert!(!wad.find_dir("P3_START").is_ok());

        let mut result = Self {
            color_map,
            palette,

            missing_texture_id: 0,
            textures: Vec::new(),
        };

        result.create_missing_texture();
        result.load_all_patches(wad);
        result.load_all_flats(wad);
        result.load_all_textures(wad);

        Some(result)
    }

    fn create_missing_texture(&mut self) {
        let mut pixels = vec![0; 2 * 2 * std::mem::size_of::<u32>()];

        let mut set_pixel = |index: usize, r, g, b| {
            pixels[index * 4 + 0] = r;
            pixels[index * 4 + 1] = g;
            pixels[index * 4 + 2] = b;
            pixels[index * 4 + 3] = 0xff;
        };

        set_pixel(0, 0x00, 0x00, 0x00);
        set_pixel(1, 0xff, 0x00, 0xff);
        set_pixel(2, 0xff, 0x00, 0xff);
        set_pixel(3, 0x00, 0x00, 0x00);

        let id = self.textures.len();
        let texture = Texture::new(TextureTyp::Texture, 2, 2, pixels);
        self.add_texture("MISSING_TEXTURE", texture);
        self.missing_texture_id = id;
    }

    fn load_all_patches(&mut self, wad: &Wad) {
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

    fn load_all_flats(&mut self, wad: &Wad) {
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

    fn load_all_textures(&mut self, wad: &Wad) {
        let patch_names =
            read_patch_names(&wad).expect("Failed to load patch names");

        let texture_defs =
            read_texture_defs(&wad).expect("Failed to read texture defs");

        let textures = process_texture_defs(self, &patch_names, &texture_defs);

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

    pub fn missing_texture(&self) -> (usize, &Texture) {
        (
            self.missing_texture_id,
            self.load_from_id(self.missing_texture_id).unwrap(),
        )
    }

    pub fn load_from_id(&self, id: usize) -> Option<&Texture> {
        self.textures.get(id).map(|o| &o.1)
    }

    pub fn get_name_from_id(&self, id: usize) -> Option<&String> {
        self.textures.get(id).map(|o| &o.0)
    }

    pub fn load_from_name(&self, name: &str) -> Option<(usize, &Texture)> {
        for (index, t) in self.textures.iter().enumerate() {
            if t.0 == name {
                return Some((index, &t.1));
            }
        }

        None
    }

    pub fn dump<P>(&self, output_dir: P)
    where
        P: AsRef<Path>,
    {
        let output_dir = PathBuf::from(output_dir.as_ref());
        assert!(output_dir.exists());

        use serde_json::{Value, json};

        let mut result = Vec::new();
        for texture in &self.textures {
            if let Some(comp) = texture.1.composition.as_ref() {
                let patches = comp
                    .patches
                    .iter()
                    .map(|patch| {
                        json!({
                            "texture_name": patch.name,
                            "origin_x": patch.origin_x,
                            "origin_y": patch.origin_y,
                        })
                    })
                    .collect::<Value>();
                result.push(json!({
                    "name": texture.0,
                    "width": texture.1.width(),
                    "height": texture.1.height(),
                    "patches": patches
                }));
            }
        }

        let text = serde_json::to_string_pretty(&result).unwrap();
        println!("{}", text);
        // panic!();

        let mut flat_output_dir = output_dir.clone();
        flat_output_dir.push("flats");

        let mut patch_output_dir = output_dir.clone();
        patch_output_dir.push("patches");

        let mut texture_output_dir = output_dir.clone();
        texture_output_dir.push("textures");

        std::fs::create_dir_all(&flat_output_dir).unwrap();
        std::fs::create_dir_all(&patch_output_dir).unwrap();
        std::fs::create_dir_all(&texture_output_dir).unwrap();

        for texture in &self.textures {
            let output_dir = match texture.1.typ() {
                TextureTyp::Flat => &flat_output_dir,
                TextureTyp::Patch => &patch_output_dir,
                TextureTyp::Texture => &texture_output_dir,
            };

            let mut path = output_dir.clone();
            path.push(&texture.0);
            path.set_extension("png");

            let data = util::write_texture_to_png(&texture.1);
            util::write_binary_file(path, &data);
        }
    }
}
