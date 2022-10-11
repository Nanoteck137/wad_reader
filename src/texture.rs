use crate::wad::Wad;

const MAX_PALETTE_COLORS: usize = 256;
const MAX_COLOR_MAPS: usize = 34;

const FLAT_TEXTURE_WIDTH: usize = 64;
const FLAT_TEXTURE_HEIGHT: usize = 64;

pub struct Texture {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Texture {
    pub fn new(width: usize, height: usize, pixels: Vec<u8>) -> Self {
        Self {
            width,
            height,
            pixels,
        }
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

        return Some(Texture::new(width, height, pixels));
    }

    None
}
