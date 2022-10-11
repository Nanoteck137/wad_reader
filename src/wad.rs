//! Module to handle WAD files

#![allow(dead_code)]

pub const LINEDEF_FLAG_IMPASSABLE: usize = 0x0001;
pub const LINEDEF_FLAG_TWO_SIDED: usize = 0x0004;
pub const LINEDEF_FLAG_UPPER_TEXTURE_UNPEGGED: usize = 0x0008;
pub const LINEDEF_FLAG_LOWER_TEXTURE_UNPEGGED: usize = 0x0010;
pub const LINEDEF_FLAG_SECRET: usize = 0x0020;
pub const LINEDEF_FLAG_BLOCKS_SOUND: usize = 0x0020;
pub const LINEDEF_FLAG_NEVER_SHOW_ON_AUTOMAP: usize = 0x0080;
pub const LINEDEF_FLAG_ALWAYS_SHOWS_ON_AUTOMAP: usize = 0x0100;

#[derive(Copy, Clone, Debug)]
pub enum Error {
    ArrayConvertionFailed,
    ConvertToUsizeFailed,
    ConvertToF32Failed,
    BytesToStrFailed,

    UnknownMagic([u8; 4]),
    NoDirFound,
    IndexOutOfRange,

    FrontSideMismatch { side: usize },
    BackSideMismatch { side: usize },
    UnknownSide { side: usize },
}

pub type Result<T> = std::result::Result<T, Error>;

const VERT_IS_GL: usize = 1 << 15;

#[derive(Copy, Clone, Debug)]
pub struct Dir {
    data_offset: usize,
    data_size: usize,
    pub name: [u8; 8],
}

pub struct Wad<'a> {
    bytes: &'a [u8],

    num_dirs: usize,
    dir_start: usize,
}

impl<'a> Wad<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let magic = &bytes[0..4];
        if magic != b"IWAD" {
            let magic: [u8; 4] =
                magic.try_into().map_err(|_| Error::ArrayConvertionFailed)?;
            return Err(Error::UnknownMagic(magic));
        }

        let num_dirs = i32::from_le_bytes(
            bytes[4..8]
                .try_into()
                .map_err(|_| Error::ArrayConvertionFailed)?,
        );
        let num_dirs: usize = num_dirs
            .try_into()
            .map_err(|_| Error::ConvertToUsizeFailed)?;

        let dir_start = i32::from_le_bytes(
            bytes[8..12]
                .try_into()
                .map_err(|_| Error::ArrayConvertionFailed)?,
        );
        let dir_start: usize = dir_start
            .try_into()
            .map_err(|_| Error::ConvertToUsizeFailed)?;

        Ok(Self {
            bytes,

            num_dirs,
            dir_start,
        })
    }

    pub fn read_dir_entry(&self, index: usize) -> Result<Dir> {
        if index >= self.num_dirs {
            return Err(Error::IndexOutOfRange);
        }

        let start = self.dir_start + index * 16;
        let bytes = &self.bytes[start..start + 16];

        let data_offset = i32::from_le_bytes(
            bytes[0..4]
                .try_into()
                .map_err(|_| Error::ArrayConvertionFailed)?,
        );
        let data_offset: usize = data_offset
            .try_into()
            .map_err(|_| Error::ConvertToUsizeFailed)?;

        let data_size = i32::from_le_bytes(
            bytes[4..8]
                .try_into()
                .map_err(|_| Error::ArrayConvertionFailed)?,
        );
        let data_size: usize = data_size
            .try_into()
            .map_err(|_| Error::ConvertToUsizeFailed)?;

        let name = &bytes[8..16];
        let name: [u8; 8] =
            name.try_into().map_err(|_| Error::ArrayConvertionFailed)?;

        Ok(Dir {
            data_offset,
            data_size,
            name,
        })
    }

    pub fn find_dir(&self, name: &str) -> Result<usize> {
        for index in 0..self.num_dirs {
            let dir_entry = self.read_dir_entry(index)?;

            let find_zero = |n: &[u8]| {
                for i in 0..n.len() {
                    if n[i] == 0 {
                        return i;
                    }
                }

                n.len()
            };

            let len = find_zero(&dir_entry.name);
            let dir_name = std::str::from_utf8(&dir_entry.name[0..len])
                .map_err(|_| Error::BytesToStrFailed)?;
            if dir_name == name {
                return Ok(index);
            }
        }

        Err(Error::NoDirFound)
    }

    pub fn read_dir(&self, index: usize) -> Result<&[u8]> {
        let dir_entry = self.read_dir_entry(index)?;

        // TODO(patrik): Check bounds

        let start = dir_entry.data_offset;
        let end = start + dir_entry.data_size;
        let data = &self.bytes[start..end];

        Ok(data)
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
}

impl Vertex {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Line {
    pub start_vertex: usize,
    pub end_vertex: usize,
}

impl Line {
    fn new(start_vertex: usize, end_vertex: usize) -> Self {
        Self {
            start_vertex,
            end_vertex,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Linedef {
    pub line: Line,
    pub flags: usize,
    pub front_sidedef: Option<usize>,
    pub back_sidedef: Option<usize>,
}

impl Linedef {
    fn new(
        line: Line,
        flags: usize,
        front_sidedef: Option<usize>,
        back_sidedef: Option<usize>,
    ) -> Self {
        Self {
            line,
            flags,
            front_sidedef,
            back_sidedef,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Sidedef {
    pub x_offset: i16,
    pub y_offset: i16,
    pub upper_texture_name: [u8; 8],
    pub middle_texture_name: [u8; 8],
    pub lower_texture_name: [u8; 8],
    pub sector: usize,
}

impl Sidedef {
    fn new(
        x_offset: i16,
        y_offset: i16,
        upper_texture_name: [u8; 8],
        middle_texture_name: [u8; 8],
        lower_texture_name: [u8; 8],
        sector: usize,
    ) -> Self {
        Self {
            x_offset,
            y_offset,
            upper_texture_name,
            middle_texture_name,
            lower_texture_name,
            sector,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sector {
    pub floor_height: f32,
    pub ceiling_height: f32,

    pub floor_texture_name: [u8; 8],
    pub ceiling_texture_name: [u8; 8],

    pub lines: Vec<Linedef>,
    pub sub_sectors: Vec<SubSector>,
}

impl Sector {
    fn new(
        floor_height: f32,
        ceiling_height: f32,
        floor_texture_name: [u8; 8],
        ceiling_texture_name: [u8; 8],
    ) -> Self {
        Self {
            floor_height,
            ceiling_height,

            floor_texture_name,
            ceiling_texture_name,

            lines: Vec::new(),
            sub_sectors: Vec::new(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SubSector {
    pub start: usize,
    pub count: usize,
}

impl SubSector {
    fn new(start: usize, count: usize) -> Self {
        Self { start, count }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Segment {
    pub start_vertex: usize,
    pub end_vertex: usize,

    pub linedef: usize,
    pub side: usize,
    pub partner_segment: usize,
}

impl Segment {
    fn new(
        start_vertex: usize,
        end_vertex: usize,
        linedef: usize,
        side: usize,
        partner_segment: usize,
    ) -> Self {
        Self {
            start_vertex,
            end_vertex,
            linedef,
            side,
            partner_segment,
        }
    }
}

pub struct Map {
    pub vertices: Vec<Vertex>,
    pub gl_vertices: Vec<Vertex>,

    pub linedefs: Vec<Linedef>,
    pub sidedefs: Vec<Sidedef>,
    pub sectors: Vec<Sector>,

    pub segments: Vec<Segment>,
    pub sub_sectors: Vec<SubSector>,
}

impl Map {
    pub fn parse_from_wad(wad: &Wad, map_name: &str) -> Result<Self> {
        let mut res = Self {
            vertices: Vec::new(),
            gl_vertices: Vec::new(),

            linedefs: Vec::new(),
            sidedefs: Vec::new(),
            sectors: Vec::new(),

            segments: Vec::new(),
            sub_sectors: Vec::new(),
        };

        let map_index = wad.find_dir(map_name)?;

        res.load_vertices(wad, map_index)?;
        res.load_linedefs(wad, map_index)?;
        res.load_sidedefs(wad, map_index)?;
        res.load_sectors(wad, map_index)?;
        res.load_subsectors(wad, map_index)?;
        res.load_segments(wad, map_index)?;

        res.sort_subsectors()?;

        Ok(res)
    }

    fn load_vertices(&mut self, wad: &Wad, map_index: usize) -> Result<()> {
        // Load the normal vertices
        {
            let data = wad.read_dir(map_index + 4)?;

            let count = data.len() / 4;

            for index in 0..count {
                let start = index * 4;
                let data = &data[start..start + 4];

                let x = i16::from_le_bytes(
                    data[0..2]
                        .try_into()
                        .map_err(|_| Error::ArrayConvertionFailed)?,
                );
                let y = i16::from_le_bytes(
                    data[2..4]
                        .try_into()
                        .map_err(|_| Error::ArrayConvertionFailed)?,
                );

                let x: f32 =
                    x.try_into().map_err(|_| Error::ConvertToF32Failed)?;
                let y: f32 =
                    y.try_into().map_err(|_| Error::ConvertToF32Failed)?;

                self.vertices.push(Vertex::new(x, y));
            }
        }

        // Load the extra vertices (GL_VERT)
        {
            let data = wad.read_dir(map_index + 12)?;

            //TODO(patrik): Make sure the gl_magic is correct
            let _gl_magic = &data[0..4];

            let data = &data[4..];

            let count = data.len() / 8;

            for index in 0..count {
                let start = index * 8;
                let data = &data[start..start + 8];

                let x = i32::from_le_bytes(
                    data[0..4]
                        .try_into()
                        .map_err(|_| Error::ArrayConvertionFailed)?,
                );
                let y = i32::from_le_bytes(
                    data[4..8]
                        .try_into()
                        .map_err(|_| Error::ArrayConvertionFailed)?,
                );

                let x = x as f32 / 65536.0;
                let y = y as f32 / 65536.0;

                self.gl_vertices.push(Vertex::new(x, y));
            }
        }

        Ok(())
    }

    fn load_linedefs(&mut self, wad: &Wad, map_index: usize) -> Result<()> {
        let data = wad.read_dir(map_index + 2)?;

        let count = data.len() / 14;

        for index in 0..count {
            let start = index * 14;
            let data = &data[start..start + 14];

            let start_vertex = i16::from_le_bytes(
                data[0..2]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );
            let end_vertex = i16::from_le_bytes(
                data[2..4]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );

            let flags = i32::from_le_bytes(
                data[4..8]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );

            let front_sidedef = i16::from_le_bytes(
                data[10..12]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );
            let back_sidedef = i16::from_le_bytes(
                data[12..14]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );

            let start_vertex: usize = start_vertex
                .try_into()
                .map_err(|_| Error::ConvertToUsizeFailed)?;
            let end_vertex: usize = end_vertex
                .try_into()
                .map_err(|_| Error::ConvertToUsizeFailed)?;

            let flags: usize =
                flags.try_into().map_err(|_| Error::ConvertToUsizeFailed)?;

            let line = Line::new(start_vertex, end_vertex);

            let front_sidedef = if front_sidedef == -1 {
                None
            } else {
                Some(
                    front_sidedef
                        .try_into()
                        .map_err(|_| Error::ConvertToUsizeFailed)?,
                )
            };

            let back_sidedef = if back_sidedef == -1 {
                None
            } else {
                Some(
                    back_sidedef
                        .try_into()
                        .map_err(|_| Error::ConvertToUsizeFailed)?,
                )
            };

            self.linedefs.push(Linedef::new(
                line,
                flags,
                front_sidedef,
                back_sidedef,
            ));
        }

        Ok(())
    }

    fn load_sidedefs(&mut self, wad: &Wad, map_index: usize) -> Result<()> {
        let data = wad.read_dir(map_index + 3)?;
        let count = data.len() / 30;

        for index in 0..count {
            let start = index * 30;
            let data = &data[start..start + 30];

            let x_offset = i16::from_le_bytes(
                data[0..2]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );

            let y_offset = i16::from_le_bytes(
                data[2..4]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );

            let upper_texture_name: [u8; 8] = data[4..12]
                .try_into()
                .map_err(|_| Error::ArrayConvertionFailed)?;

            let lower_texture_name: [u8; 8] = data[12..20]
                .try_into()
                .map_err(|_| Error::ArrayConvertionFailed)?;

            let middle_texture_name: [u8; 8] = data[20..28]
                .try_into()
                .map_err(|_| Error::ArrayConvertionFailed)?;

            let sector = i16::from_le_bytes(
                data[28..30]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );

            let sector: usize =
                sector.try_into().map_err(|_| Error::ConvertToUsizeFailed)?;

            self.sidedefs.push(Sidedef::new(
                x_offset,
                y_offset,
                upper_texture_name,
                middle_texture_name,
                lower_texture_name,
                sector,
            ));
        }

        Ok(())
    }

    fn load_sectors(&mut self, wad: &Wad, map_index: usize) -> Result<()> {
        let data = wad.read_dir(map_index + 8)?;
        let count = data.len() / 26;

        for index in 0..count {
            let start = index * 26;
            let data = &data[start..start + 26];

            let floor_height = i16::from_le_bytes(
                data[0..2]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );
            let ceiling_height = i16::from_le_bytes(
                data[2..4]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );

            let floor_height: f32 = floor_height
                .try_into()
                .map_err(|_| Error::ConvertToF32Failed)?;

            let ceiling_height: f32 = ceiling_height
                .try_into()
                .map_err(|_| Error::ConvertToF32Failed)?;

            let floor_texture_name: [u8; 8] = data[4..12]
                .try_into()
                .map_err(|_| Error::ArrayConvertionFailed)?;
            let ceiling_texture_name: [u8; 8] = data[12..20]
                .try_into()
                .map_err(|_| Error::ArrayConvertionFailed)?;

            self.sectors.push(Sector::new(
                floor_height,
                ceiling_height,
                floor_texture_name,
                ceiling_texture_name,
            ));
        }

        Ok(())
    }

    fn load_subsectors(&mut self, wad: &Wad, map_index: usize) -> Result<()> {
        let data = wad.read_dir(map_index + 14)?;
        // TODO(patrik): Look for magic

        let count = data.len() / 4;
        for index in 0..count {
            let start = index * 4;
            let data = &data[start..start + 4];

            let count = u16::from_le_bytes(
                data[0..2]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );
            let start = u16::from_le_bytes(
                data[2..4]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );

            let start: usize =
                start.try_into().map_err(|_| Error::ConvertToUsizeFailed)?;
            let count: usize =
                count.try_into().map_err(|_| Error::ConvertToUsizeFailed)?;

            self.sub_sectors.push(SubSector::new(start, count));
        }

        Ok(())
    }

    fn load_segments(&mut self, wad: &Wad, map_index: usize) -> Result<()> {
        let data = wad.read_dir(map_index + 13)?;
        // TODO(patrik): Look for magic

        let count = data.len() / 10;

        for index in 0..count {
            let start = index * 10;
            let data = &data[start..start + 10];

            let start_vertex = u16::from_le_bytes(
                data[0..2]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );
            let end_vertex = u16::from_le_bytes(
                data[2..4]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );

            let linedef = u16::from_le_bytes(
                data[4..6]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );
            let side = u16::from_le_bytes(
                data[6..8]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );
            let partner_segment = u16::from_le_bytes(
                data[8..10]
                    .try_into()
                    .map_err(|_| Error::ArrayConvertionFailed)?,
            );

            let start_vertex: usize = start_vertex
                .try_into()
                .map_err(|_| Error::ConvertToUsizeFailed)?;

            let end_vertex: usize = end_vertex
                .try_into()
                .map_err(|_| Error::ConvertToUsizeFailed)?;

            let linedef: usize = linedef
                .try_into()
                .map_err(|_| Error::ConvertToUsizeFailed)?;

            let side: usize =
                side.try_into().map_err(|_| Error::ConvertToUsizeFailed)?;

            let partner_segment: usize = partner_segment
                .try_into()
                .map_err(|_| Error::ConvertToUsizeFailed)?;

            self.segments.push(Segment::new(
                start_vertex,
                end_vertex,
                linedef,
                side,
                partner_segment,
            ));
        }

        Ok(())
    }

    fn sort_subsectors(&mut self) -> Result<()> {
        for line in &self.linedefs {
            let sector = if let Some(side) = line.front_sidedef {
                let side = self.sidedefs[side];
                Ok(side.sector)
            } else if let Some(side) = line.back_sidedef {
                let side = self.sidedefs[side];
                Ok(side.sector)
            } else {
                continue;
            }?;

            self.sectors[sector].lines.push(*line);
        }

        for sub_sector in &self.sub_sectors {
            let segment = self.segments[sub_sector.start];
            if segment.linedef != 0xffff {
                let linedef = self.linedefs[segment.linedef];
                let sidedef = if segment.side == 0 {
                    linedef
                        .front_sidedef
                        .ok_or(Error::FrontSideMismatch { side: segment.side })
                } else if segment.side == 1 {
                    linedef
                        .back_sidedef
                        .ok_or(Error::BackSideMismatch { side: segment.side })
                } else {
                    Err(Error::UnknownSide { side: segment.side })
                }?;

                let sidedef = self.sidedefs[sidedef];
                self.sectors[sidedef.sector].sub_sectors.push(*sub_sector);
            }
        }

        Ok(())
    }

    pub fn vertex(&self, index: usize) -> Vertex {
        return if index & VERT_IS_GL == VERT_IS_GL {
            self.gl_vertices[index & !VERT_IS_GL]
        } else {
            self.vertices[index]
        };
    }
}
