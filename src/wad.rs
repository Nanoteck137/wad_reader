//! Module to handle WAD files

#[derive(Copy, Clone, Debug)]
pub enum Error {
    ArrayConvertionFailed,
    ConvertToUsizeFailed,
    BytesToStrFailed,

    UnknownMagic([u8; 4]),
    NoDirFound,
    IndexOutOfRange,
}

pub type Result<T> = std::result::Result<T, Error>;

const VERT_IS_GL: usize = 1 << 15;

#[derive(Copy, Clone, Debug)]
pub struct Dir {
    data_offset: usize,
    data_size: usize,
    name: [u8; 8],
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
            bytes[4..8].try_into().map_err(|_| Error::ArrayConvertionFailed)?);
        let num_dirs: usize = num_dirs.try_into()
            .map_err(|_| Error::ConvertToUsizeFailed)?;

        let dir_start = i32::from_le_bytes(
            bytes[8..12].try_into()
                .map_err(|_| Error::ArrayConvertionFailed)?);
        let dir_start: usize =
            dir_start.try_into().map_err(|_| Error::ConvertToUsizeFailed)?;

        Ok(Self {
            bytes,

            num_dirs,
            dir_start
        })
    }

    pub fn read_dir_entry(&self, index: usize) -> Result<Dir> {
        if index >= self.num_dirs {
            return Err(Error::IndexOutOfRange);
        }

        let start = self.dir_start + index * 16;
        let bytes = &self.bytes[start..start + 16];

        let data_offset = i32::from_le_bytes(
            bytes[0..4].try_into().map_err(|_| Error::ArrayConvertionFailed)?);
        let data_offset: usize =
            data_offset.try_into().map_err(|_| Error::ConvertToUsizeFailed)?;

        let data_size = i32::from_le_bytes(
            bytes[4..8].try_into().map_err(|_| Error::ArrayConvertionFailed)?);
        let data_size: usize =
            data_size.try_into().map_err(|_| Error::ConvertToUsizeFailed)?;

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
            let dir_name =
                std::str::from_utf8(&dir_entry.name[0..len])
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

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Line {
    pub start_vertex: usize,
    pub end_vertex: usize,
}

#[derive(Copy, Clone, Debug)]
pub struct Linedef {
    pub line: Line,
    pub front_sidedef: Option<usize>,
    pub back_sidedef: Option<usize>,
}

#[derive(Copy, Clone, Debug)]
pub struct Sidedef {
    pub sector: usize,
}

#[derive(Clone, Debug)]
pub struct Sector {
    pub floor_height: usize,
    pub ceiling_height: usize,
    pub lines: Vec<Linedef>,
    pub sub_sectors: Vec<SubSector>,
}

#[derive(Copy, Clone, Debug)]
pub struct SubSector {
    pub start: usize,
    pub count: usize,
}

#[derive(Copy, Clone, Debug)]
pub struct Segment {
    pub start_vertex: usize,
    pub end_vertex: usize,

    pub linedef: usize,
    pub side: usize,
    pub partner_segment: usize,
}

pub struct Map {
    pub vertices: Vec<Vertex>,
    pub gl_vertices: Vec<Vertex>,

    pub lines: Vec<Linedef>,
    pub sidedefs: Vec<Sidedef>,
    pub sectors: Vec<Sector>,

    pub segments: Vec<Segment>,
    pub sub_sectors: Vec<SubSector>,
}

impl Map {
    pub fn parse_from_wad(wad: &Wad, map_name: &str) -> Result<Self> {
        let res = Self {
            vertices: Vec::new(),
            gl_vertices: Vec::new(),

            lines: Vec::new(),
            sidedefs: Vec::new(),
            sectors: Vec::new(),

            segments: Vec::new(),
            sub_sectors: Vec::new(),
        };

        let map_index = wad.find_dir(map_name)?;

        Ok(res)
    }

    pub fn vertex(&self, index: usize) -> Vertex {
        return if index & VERT_IS_GL == VERT_IS_GL {
            self.gl_vertices[index & !VERT_IS_GL]
        } else {
            self.vertices[index]
        };
    }
}
