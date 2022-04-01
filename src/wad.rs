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

                let x = i16::from_le_bytes(data[0..2].try_into().unwrap());
                let y = i16::from_le_bytes(data[2..4].try_into().unwrap());

                self.vertices.push(Vertex {
                    x: x.try_into().unwrap(),
                    y: y.try_into().unwrap(),
                });
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

                let x = i32::from_le_bytes(data[0..4].try_into().unwrap());
                let y = i32::from_le_bytes(data[4..8].try_into().unwrap());

                let x = x as f32 / 65536.0;
                let y = y as f32 / 65536.0;

                self.gl_vertices.push(Vertex {
                    x: x,
                    y: y,
                });
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

            let start_vertex = i16::from_le_bytes(data[0..2].try_into().unwrap());
            let end_vertex = i16::from_le_bytes(data[2..4].try_into().unwrap());

            let front_sidedef = i16::from_le_bytes(data[10..12].try_into().unwrap());
            let back_sidedef = i16::from_le_bytes(data[12..14].try_into().unwrap());

            self.linedefs.push(Linedef {
                line: Line {
                    start_vertex: start_vertex.try_into().unwrap(),
                    end_vertex: end_vertex.try_into().unwrap(),
                },
                front_sidedef: if front_sidedef == -1 { None } else { Some(front_sidedef.try_into().unwrap()) },
                back_sidedef: if back_sidedef == -1 { None } else { Some(back_sidedef.try_into().unwrap()) },
            });
        }

        Ok(())
    }

    fn load_sidedefs(&mut self, wad: &Wad, map_index: usize) -> Result<()> {
        let data = wad.read_dir(map_index + 3)?;
        let count = data.len() / 30;

        for index in 0..count {
            let start = index * 30;
            let data = &data[start..start + 30];

            let sector = i16::from_le_bytes(data[28..30].try_into().unwrap());
            self.sidedefs.push(Sidedef {
                sector: sector.try_into().unwrap(),
            });
        }

        Ok(())
    }

    fn load_sectors(&mut self, wad: &Wad, map_index: usize) -> Result<()> {
        let data = wad.read_dir(map_index + 3)?;
        let count = data.len() / 26;

        for index in 0..count {
            let start = index * 26;
            let data = &data[start..start + 26];

            let floor_height = i16::from_le_bytes(data[0..2].try_into().unwrap());
            let ceiling_height = i16::from_le_bytes(data[2..4].try_into().unwrap());

            self.sectors.push(Sector {
                floor_height: floor_height.try_into().unwrap(),
                ceiling_height: ceiling_height.try_into().unwrap(),
                lines: Vec::new(),
                sub_sectors: Vec::new(),
            });
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

            let count = u16::from_le_bytes(data[0..2].try_into().unwrap());
            let start = u16::from_le_bytes(data[2..4].try_into().unwrap());

            self.sub_sectors.push(SubSector {
                start: start.try_into().unwrap(),
                count: count.try_into().unwrap(),
            });
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

            let start_vertex = u16::from_le_bytes(data[0..2].try_into().unwrap());
            let end_vertex = u16::from_le_bytes(data[2..4].try_into().unwrap());

            let linedef = u16::from_le_bytes(data[4..6].try_into().unwrap());
            let side = u16::from_le_bytes(data[6..8].try_into().unwrap());
            let partner_segment = u16::from_le_bytes(data[8..10].try_into().unwrap());

            self.segments.push(Segment {
                start_vertex: start_vertex.try_into().unwrap(),
                end_vertex: end_vertex.try_into().unwrap(),

                linedef: linedef.try_into().unwrap(),
                side: side.try_into().unwrap(),
                partner_segment: partner_segment.try_into().unwrap(),
            });
        }

        Ok(())
    }

    fn sort_subsectors(&mut self) -> Result<()> {
        for sub_sector in &self.sub_sectors {
            let segment = self.segments[sub_sector.start];
            if segment.linedef != 0xffff {
                let linedef = self.linedefs[segment.linedef];
                let sidedef = if segment.side == 0 {
                    linedef.front_sidedef.unwrap()
                } else if segment.side == 1 {
                    linedef.back_sidedef.unwrap()
                } else {
                    // TODO(patrik): Make error
                    panic!("Unknown segment side: {}", segment.side);
                };

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
