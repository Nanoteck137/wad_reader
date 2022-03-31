use std::collections::HashSet;
use std::path::Path;
use std::fs::File;
use std::io::Read;


use glutin_window::GlutinWindow as Window;
use opengl_graphics::{GlGraphics, OpenGL};
use piston::event_loop::{EventSettings, Events};
use piston::input::{RenderArgs, RenderEvent, UpdateArgs, UpdateEvent, PressEvent, ReleaseEvent, Key, Button};
use piston::window::WindowSettings;
// use rgeometry::data::{ Polygon, Point };
use delaunator::{Point};

const VERT_IS_GL: usize = (1 << 15);

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

#[derive(Copy, Clone, PartialEq, Debug)]
struct MyVertex {
    x: f64,
    y: f64,
}

impl std::ops::Sub for MyVertex {
    type Output = MyVertex;

    fn sub(self, rhs: MyVertex) -> Self {
        let x = self.x - rhs.x;
        let y = self.y - rhs.y;

        MyVertex { x, y }
    }

}

#[derive(Copy, Clone, PartialEq, Debug)]
struct MyLine {
    start_vertex: usize,
    end_vertex: usize,
}

#[derive(Copy, Clone, Debug)]
struct MyLineDef {
    line: MyLine,
    front_sidedef: Option<usize>,
    back_sidedef: Option<usize>,
}

#[derive(Copy, Clone, Debug)]
struct MySidedef {
    sector: usize,
}

#[derive(Clone, Debug)]
struct MySector {
    floor_height: usize,
    ceiling_height: usize,
    lines: Vec<MyLineDef>,
    sub_sectors: Vec<MySubSector>,

    box_start: MyVertex,
    box_end: MyVertex,
}

#[derive(Copy, Clone, Debug)]
struct MySubSector {
    start: usize,
    count: usize,
}

#[derive(Copy, Clone, Debug)]
struct MySegment {
    start_vertex: usize,
    end_vertex: usize,

    linedef: usize,
    side: usize,
    partner_segment: usize,
}

#[derive(Copy, Clone, Debug)]
struct MyBox {
    min_x: f64,
    min_y: f64,

    max_x: f64,
    max_y: f64,
}

#[derive(Copy, Clone, Debug)]
struct MyNode {
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,

    right_box: MyBox,
    left_box: MyBox,

    right_child: usize,
    left_child: usize,
}

pub struct App {
    gl: GlGraphics, // OpenGL drawing backend.
    camera_x: f64,
    camera_y: f64,
    zoom: f64,

    left: bool,
    right: bool,
    up: bool,
    down: bool,
    zoom_in: bool,
    zoom_out: bool,

    sub_sector_index: usize,
    vertices: Vec<MyVertex>,
    lines: Vec<MyLineDef>,
    sidedefs: Vec<MySidedef>,
    sectors: Vec<MySector>,
    sub_sectors: Vec<MySubSector>,
    segments: Vec<MySegment>,

    // GL
    gl_vertices: Vec<MyVertex>,
    gl_segments: Vec<MySegment>,
    gl_sub_sectors: Vec<MySubSector>,
    gl_nodes: Vec<MyNode>,

    test_segments: Vec<MySegment>,
}

fn read_file<P>(path: P) -> Vec<u8>
    where P: AsRef<Path>
{
    let mut file = File::open(path).unwrap();

    let mut result = Vec::new();
    file.read_to_end(&mut result).unwrap();

    result
}

#[derive(Copy, Clone, Debug)]
struct WadDir {
    data_offset: usize,
    data_size: usize,
    name: [u8; 8],
}

struct Wad<'a> {
    bytes: &'a [u8],

    num_dirs: usize,
    dir_start: usize,
}

impl<'a> Wad<'a> {
    fn parse(bytes: &'a [u8]) -> Option<Self> {
        let magic = &bytes[0..4];
        if magic != b"IWAD" {
            return None;
        }

        let num_dirs = i32::from_le_bytes(bytes[4..8].try_into().ok()?);
        let num_dirs: usize = num_dirs.try_into().ok()?;

        let dir_start = i32::from_le_bytes(bytes[8..12].try_into().ok()?);
        let dir_start: usize = dir_start.try_into().ok()?;

        Some(Self {
            bytes,

            num_dirs,
            dir_start
        })
    }

    fn read_dir_entry(&self, index: usize) -> Option<WadDir> {
        if index >= self.num_dirs {
            return None;
        }

        let start = self.dir_start + index * 16;
        let bytes = &self.bytes[start..start + 16];

        let data_offset = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let data_offset: usize = data_offset.try_into().ok()?;

        let data_size = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let data_size: usize = data_size.try_into().ok()?;

        let name = &bytes[8..16];

        Some(WadDir {
            data_offset,
            data_size,
            name: name.try_into().ok()?,
        })
    }

    fn find_dir(&self, name: &str) -> Option<usize> {
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
            let dir_name = std::str::from_utf8(&dir_entry.name[0..len]).ok()?;
            if dir_name == name {
                return Some(index);
            }
        }

        None
    }

    fn read_dir(&self, index: usize) -> Option<&[u8]> {
        let dir_entry = self.read_dir_entry(index)?;

        let start = dir_entry.data_offset;
        let end = start + dir_entry.data_size;
        let data = &self.bytes[start..end];

        Some(data)
    }
}

fn test_wad_data(app: &mut App) {
    let data = read_file("doom1.wad");
    let wad = Wad::parse(&data).unwrap();

    let index = wad.find_dir("E1M1").unwrap();

    {
        let vertices = wad.read_dir(index + 4).unwrap();

        let num_vertices = vertices.len() / 4;
        println!("Num vertices: {}", num_vertices);

        for index in 0..num_vertices {
            let start = index * 4;
            let data = &vertices[start..start + 4];

            let x = i16::from_le_bytes(data[0..2].try_into().unwrap());
            let y = i16::from_le_bytes(data[2..4].try_into().unwrap());

            app.vertices.push(MyVertex {
                x: x.try_into().unwrap(),
                y: y.try_into().unwrap(),
            });
        }
    }

    {
        let lines = wad.read_dir(index + 2).unwrap();

        let num_lines = lines.len() / 14;
        println!("Num lines: {}", num_lines);

        for index in 0..num_lines {
            let start = index * 14;
            let data = &lines[start..start + 14];

            let start_vertex = i16::from_le_bytes(data[0..2].try_into().unwrap());
            let end_vertex = i16::from_le_bytes(data[2..4].try_into().unwrap());

            let front_sidedef = i16::from_le_bytes(data[10..12].try_into().unwrap());
            let back_sidedef = i16::from_le_bytes(data[12..14].try_into().unwrap());

            app.lines.push(MyLineDef {
                line: MyLine {
                    start_vertex: start_vertex.try_into().unwrap(),
                    end_vertex: end_vertex.try_into().unwrap(),
                },
                front_sidedef: if front_sidedef == -1 { None } else { Some(front_sidedef.try_into().unwrap()) },
                back_sidedef: if back_sidedef == -1 { None } else { Some(back_sidedef.try_into().unwrap()) },
            });
        }
    }

    {
        let data = wad.read_dir(index + 3).unwrap();

        let len = data.len() / 26;
        println!("Num sectors: {}", len);

        for index in 0..len {
            let start = index * 26;
            let data = &data[start..start + 26];

            let floor_height = i16::from_le_bytes(data[0..2].try_into().unwrap());
            let ceiling_height = i16::from_le_bytes(data[2..4].try_into().unwrap());

            app.sectors.push(MySector {
                floor_height: floor_height.try_into().unwrap(),
                ceiling_height: ceiling_height.try_into().unwrap(),
                lines: Vec::new(),
                sub_sectors: Vec::new(),

                box_start: MyVertex { x: 0.0, y: 0.0 },
                box_end: MyVertex { x: 0.0, y: 0.0 },
            });
        }
    }

    {
        let data = wad.read_dir(index + 3).unwrap();

        let len = data.len() / 30;
        println!("Num sidedefs: {}", len);

        for index in 0..len {
            let start = index * 30;
            let data = &data[start..start + 30];

            let sector = i16::from_le_bytes(data[28..30].try_into().unwrap());
            app.sidedefs.push(MySidedef {
                sector: sector.try_into().unwrap(),
            });
        }
    }

    {
        let data = wad.read_dir(index + 6).unwrap();

        let len = data.len() / 4;
        println!("Num sub-sectors: {}", len);

        for index in 0..len {
            let start = index * 4;
            let data = &data[start..start + 4];

            let count = i16::from_le_bytes(data[0..2].try_into().unwrap());
            let segment = i16::from_le_bytes(data[2..4].try_into().unwrap());

            app.sub_sectors.push(MySubSector {
                start: start.try_into().unwrap(),
                count: count.try_into().unwrap(),
            });
        }
    }

    // Parse the segments
    {
        let segments = wad.read_dir(index + 5).unwrap();

        let num_segments = segments.len() / 12;
        println!("Num segments: {}", num_segments);

        for index in 0..num_segments {
            let start = index * 12;
            let data = &segments[start..start + 12];

            let start_vertex = i16::from_le_bytes(data[0..2].try_into().unwrap());
            let end_vertex = i16::from_le_bytes(data[2..4].try_into().unwrap());

            let line_index = i16::from_le_bytes(data[6..8].try_into().unwrap());
            let offset = i16::from_le_bytes(data[10..12].try_into().unwrap());

            app.segments.push(MySegment {
                start_vertex: start_vertex.try_into().unwrap(),
                end_vertex: end_vertex.try_into().unwrap(),

                linedef: line_index.try_into().unwrap(),
                side: 0,
                partner_segment: 0,
            });
        }
    }

    // Parse the GL_VERT
    {
        let data = wad.read_dir(index + 12).unwrap();

        let gl_magic = &data[0..4];
        println!("GL_VERT Magic: {:?}", std::str::from_utf8(&gl_magic));

        let data = &data[4..];

        // -4 because there is a 4 byte magic
        let len = data.len() / 8;
        println!("Num GL_VERT: {}", len);

        for index in 0..len {
            let start = index * 8;
            let data = &data[start..start + 8];

            let x = i32::from_le_bytes(data[0..4].try_into().unwrap());
            let y = i32::from_le_bytes(data[4..8].try_into().unwrap());

            let x = x as f64 / 65536.0;
            let y = y as f64 / 65536.0;

            app.gl_vertices.push(MyVertex {
                x: x,
                y: y,
            });
        }
    }

    // Parse the GL_SSECT
    {
        let data = wad.read_dir(index + 14).unwrap();
        // TODO(patrik): Look for magic

        let len = data.len() / 4;
        println!("Num GL_SSECT: {}", len);

        for index in 0..len {
            let start = index * 4;
            let data = &data[start..start + 4];

            let count = u16::from_le_bytes(data[0..2].try_into().unwrap());
            let start = u16::from_le_bytes(data[2..4].try_into().unwrap());

            app.gl_sub_sectors.push(MySubSector {
                start: start.try_into().unwrap(),
                count: count.try_into().unwrap(),
            });
        }
    }

    // Parse the GL_SEGS
    {
        let data = wad.read_dir(index + 13).unwrap();
        // TODO(patrik): Look for magic

        let len = data.len() / 10;
        println!("Num GL_SEGS: {}", len);

        for index in 0..len {
            let start = index * 10;
            let data = &data[start..start + 10];

            let start_vertex = u16::from_le_bytes(data[0..2].try_into().unwrap());
            let end_vertex = u16::from_le_bytes(data[2..4].try_into().unwrap());

            let linedef = u16::from_le_bytes(data[4..6].try_into().unwrap());
            let side = u16::from_le_bytes(data[6..8].try_into().unwrap());
            let partner_segment = u16::from_le_bytes(data[8..10].try_into().unwrap());

            app.gl_segments.push(MySegment {
                start_vertex: start_vertex.try_into().unwrap(),
                end_vertex: end_vertex.try_into().unwrap(),

                linedef: linedef.try_into().unwrap(),
                side: side.try_into().unwrap(),
                partner_segment: partner_segment.try_into().unwrap(),
            });
        }
    }

    // Parse the GL_NODES
    {
        let data = wad.read_dir(index + 15).unwrap();
        // TODO(patrik): Look for magic

        let len = data.len() / 28;
        println!("Num GL_NODES: {}", len);

        for index in 0..len {
            let start = index * 28;
            let data = &data[start..start + 28];

            let x = i16::from_le_bytes(data[0..2].try_into().unwrap());
            let y = i16::from_le_bytes(data[2..4].try_into().unwrap());
            let dx = i16::from_le_bytes(data[4..6].try_into().unwrap());
            let dy = i16::from_le_bytes(data[6..8].try_into().unwrap());

            let parse_box = |b: &[u8]| {
                let max_y = i16::from_le_bytes(b[0..2].try_into().unwrap());
                let min_y = i16::from_le_bytes(b[2..4].try_into().unwrap());
                let min_x = i16::from_le_bytes(b[4..6].try_into().unwrap());
                let max_x = i16::from_le_bytes(b[6..8].try_into().unwrap());

                MyBox {
                    min_x: min_x.try_into().unwrap(),
                    min_y: min_y.try_into().unwrap(),
                    max_x: max_x.try_into().unwrap(),
                    max_y: max_y.try_into().unwrap(),
                }
            };

            let right_box = parse_box(&data[8..16]);
            let left_box = parse_box(&data[16..24]);

            let right_child = u16::from_le_bytes(data[24..26].try_into().unwrap());
            let left_child = u16::from_le_bytes(data[26..28].try_into().unwrap());

            app.gl_nodes.push(MyNode {
                x: x.try_into().unwrap(),
                y: y.try_into().unwrap(),
                dx: dx.try_into().unwrap(),
                dy: dy.try_into().unwrap(),

                right_box,
                left_box,

                right_child: right_child.try_into().unwrap(),
                left_child: left_child.try_into().unwrap(),
            });
        }
    }

    let mut mini_segments = Vec::new();

    for sub_sector in &app.gl_sub_sectors {
        let mut normal_segments = Vec::new();

        for i in 0..sub_sector.count {
            let segment = app.gl_segments[sub_sector.start + i];

            if segment.linedef != 0xffff {
                normal_segments.push((i, segment));
            } else {
                mini_segments.push((i, segment));
            }
        }

        for (_segment_index, segment) in &normal_segments {
            let linedef = app.lines[segment.linedef];
            let sidedef = if segment.side == 0 {
                linedef.front_sidedef.unwrap()
            } else if segment.side == 1 {
                linedef.back_sidedef.unwrap()
            } else {
                panic!("wot");
            };

            let sidedef = app.sidedefs[sidedef];
            app.sectors[sidedef.sector].lines.push(linedef);
            // app.sectors[sidedef.sector].segments.push(*segment);
        }

        // TODO(patrik): Look at the normal segments inside here
        // And find their sector to live in
        // Then look at the mini segments and add them also to the same sector

        // TODO(patrik): We need to find if minisegs are inside sectors
        // We could test and see if ray-casting polygon
        // https://wrf.ecse.rpi.edu/Research/Short_Notes/pnpoly.html
        // https://stackoverflow.com/questions/217578/how-can-i-determine-whether-a-2d-point-is-within-a-polygon

        // println!("Normal Segments: {}", normal_segments.len());
        // println!("Mini Segments: {}", mini_segments.len());
    }

    let min_val = |a: f64, b: f64| {
        if a < b { a } else { b }
    };

    let max_val = |a: f64, b: f64| {
        if a > b { a } else { b }
    };

    let min_vert = |a: MyVertex, b: MyVertex| {
        MyVertex {
            x: min_val(a.x, b.x),
            y: min_val(a.y, b.y),
        }
    };

    let max_vert = |a: MyVertex, b: MyVertex| {
        MyVertex {
            x: max_val(a.x, b.x),
            y: max_val(a.y, b.y),
        }
    };

    let bounding_box = |sector: &MySector| {
        let mut min = MyVertex { x: f64::MAX, y: f64::MAX };
        let mut max = MyVertex { x: f64::MIN, y: f64::MIN };

        for line in &sector.lines {
            let line = line.line;
            let start = if line.start_vertex & VERT_IS_GL == VERT_IS_GL {
                app.gl_vertices[line.start_vertex & !VERT_IS_GL]
            } else {
                app.vertices[line.start_vertex]
            };

            let end = if line.end_vertex & VERT_IS_GL == VERT_IS_GL {
                app.gl_vertices[line.end_vertex & !VERT_IS_GL]
            } else {
                app.vertices[line.end_vertex]
            };

            min = min_vert(start, min);
            min = min_vert(end, min);

            max = max_vert(start, max);
            max = max_vert(end, max);
        }

        (min, max)
    };

    for sector in app.sectors.iter_mut() {
        let (min, max) = bounding_box(sector);

        (*sector).box_start = min;
        (*sector).box_end = max;
    }

    // Fix subsectors

    let mut used_sub_sectors = 0;
    for sub_sector in &app.gl_sub_sectors {
        let segment = app.gl_segments[sub_sector.start];
        if segment.linedef != 0xffff {
            let linedef = app.lines[segment.linedef];
            let sidedef = if segment.side == 0 {
                linedef.front_sidedef.unwrap()
            } else if segment.side == 1 {
                linedef.back_sidedef.unwrap()
            } else {
                panic!("wot");
            };

            let sidedef = app.sidedefs[sidedef];
            app.sectors[sidedef.sector].sub_sectors.push(*sub_sector);
        }
    }

    /*
    //let sector = &mut app.sectors[29];
    for sector in app.sectors.iter_mut() {
        let mut polygon = Vec::new();
        for segment in &sector.segments {
            let line = app.lines[segment.linedef];
            let line = line.line;

            let start = if line.start_vertex & VERT_IS_GL == VERT_IS_GL {
                app.gl_vertices[line.start_vertex & !VERT_IS_GL]
            } else {
                app.vertices[line.start_vertex]
            };

            let end = if line.end_vertex & VERT_IS_GL == VERT_IS_GL {
                app.gl_vertices[line.end_vertex & !VERT_IS_GL]
            } else {
                app.vertices[line.end_vertex]
            };

            polygon.push(start);
            polygon.push(end);
        }

        // polygon.dedup();

        println!("Before Segments: {}", sector.segments.len());

        for (index, segment) in &mini_segments {
            let start = if segment.start_vertex & VERT_IS_GL == VERT_IS_GL {
                app.gl_vertices[segment.start_vertex & !VERT_IS_GL]
            } else {
                app.vertices[segment.start_vertex]
            };

            let end = if segment.end_vertex & VERT_IS_GL == VERT_IS_GL {
                app.gl_vertices[segment.end_vertex & !VERT_IS_GL]
            } else {
                app.vertices[segment.end_vertex]
            };

            if point_in_polygon(&polygon, start) && point_in_polygon(&polygon, end) {
                sector.segments.push(*segment);
            }
        }

        println!("After Segments: {}", sector.segments.len());
    }
    */

}

fn point_in_polygon(verts: &Vec<MyVertex>, point: MyVertex) -> bool {
    let mut angle = 0.0;

    for i in 0..verts.len() {
        let p1 = MyVertex {
            x: verts[i].x - point.x,
            y: verts[i].y - point.y,
        };

        let p2 = MyVertex {
            x: verts[(i + 1) % verts.len()].x - point.x,
            y: verts[(i + 1) % verts.len()].y - point.y
        };

        angle += angle_2d(p1.x, p1.y, p2.x, p2.y);
    }

    return if angle.abs() < std::f64::consts::PI {
        false
    }
    else {
        true
    };
}

fn angle_2d(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
   let theta1 = y1.atan2(x1);
   let theta2 = y2.atan2(x2);
   let mut dtheta = theta2 - theta1;

   while dtheta > std::f64::consts::PI {
      dtheta -= std::f64::consts::PI * 2.0;
   }

   while dtheta < -std::f64::consts::PI {
      dtheta += std::f64::consts::PI * 2.0;
   }

   dtheta
}


fn index_vec<T>(v: &Vec<T>, i: isize) -> T
    where T: Copy
{
    let len: isize = v.len().try_into().unwrap();

    return if i >= len as isize{
        v[(i % len) as usize]
    } else if i < 0 {
        v[(i % len + len) as usize]
    } else {
        v[i as usize]
    };
}

fn cross(a: MyVertex, b: MyVertex) -> f64 {
    return a.x * b.y - a.y * b.x;
}

fn magnitude(a: MyVertex) -> f64 {
    return (a.x * a.x + a.y * a.y).sqrt()
}

fn point_in_triangle(p: MyVertex, a: MyVertex, b: MyVertex, c: MyVertex) -> bool {
    let ab = b - a;
    let bc = c - b;
    let ca = a - c;

    let ap = p - a;
    let bp = p - b;
    let cp = p - c;

    let c1 = cross(ab, ap);
    let c2 = cross(bc, bp);
    let c3 = cross(ca, cp);

    if c1 > 0.0 || c2 > 0.0 || c3 > 0.0 {
        return false;
    }

    true
}

fn triangulate(polygon: &Vec<MyVertex>) -> Option<Vec<MyVertex>> {
    let mut vertices = Vec::new();

    let mut polygon = polygon.iter();

    let fp = match polygon.next() {
        Some(val) => val,
        None => return None,
    };

    let mut gp = match polygon.next() {
        Some(val) => val,
        None => return None,
    };

    loop {
        vertices.push(*fp);

        let p = match polygon.next() {
            Some(val) => val,
            None => break,
        };

        vertices.push(*gp);
        vertices.push(*p);
        gp = p;
    }

    Some(vertices)
}

fn line_angle(a: MyVertex, b: MyVertex) -> f64 {
    (b.y - a.y).atan2(b.x - a.x)
}

fn point_on_line(a: MyVertex, b: MyVertex, c: MyVertex) -> bool {
    return (line_angle(a, b) - line_angle(b, c)).abs() < 0.05
}

fn cleanup_lines(verts: &mut Vec<MyVertex>) {
    println!("Before: {}", verts.len());

    for mut i in 0..(verts.len() as isize) {
        let p1 = index_vec(verts, i);
        let p2 = index_vec(verts, i.wrapping_add(1));
        let p3 = index_vec(verts, i.wrapping_add(2));

        if point_on_line(p1, p2, p3) {
            verts.remove((i.wrapping_add(1) as usize) % verts.len());
            i -= 1;
        }
    }

    println!("After: {}", verts.len());
}

impl App {
    fn render(&mut self, args: &RenderArgs) {
        use graphics::*;
        use graphics::math::identity;

        const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
        const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

        let square = rectangle::square(0.0, 0.0, 10.0);

        let mut viewport = args.viewport();

        self.gl.draw(viewport, |c, mut gl| {
            // Clear the screen.
            clear([0.0, 0.0, 0.0, 1.0], gl);

            let ptr = std::ptr::addr_of_mut!(gl);

            let view = c.view.trans(-self.camera_x, self.camera_y).zoom(self.zoom);

            let mut draw_line = |l: MyLine, s, c| {
                let start = if l.start_vertex & VERT_IS_GL == VERT_IS_GL {
                    self.gl_vertices[l.start_vertex & !VERT_IS_GL]
                } else {
                    self.vertices[l.start_vertex]
                };

                let end = if l.end_vertex & VERT_IS_GL == VERT_IS_GL {
                    self.gl_vertices[l.end_vertex & !VERT_IS_GL]
                } else {
                    self.vertices[l.end_vertex]
                };

                line_from_to(c, s, [start.x as f64, start.y as f64], [end.x as f64, end.y as f64], view, unsafe { *ptr });
            };

            let mut draw_line_p = |x1, y1, x2, y2, s, c| {
                line_from_to(c, s, [x1, y1], [x2, y2], view, unsafe { *ptr });
            };

            let mut draw_vertex = |v: MyVertex, c| {
                let x: f64 = v.x.into();
                let y: f64 = v.y.into();
                let transform = identity().trans(x - 5.0, y - 5.0);

                ellipse(c, square, view.append_transform(transform), unsafe { *ptr });
            };

            let mut draw_box = |b: MyBox, c| {
                let min_x = b.min_x;
                let min_y = b.min_y;
                let max_x = b.max_x;
                let max_y = b.max_y;

                draw_vertex(MyVertex { x: min_x, y: min_y }, c);
                draw_vertex(MyVertex { x: max_x, y: min_y }, c);
                draw_vertex(MyVertex { x: max_x, y: max_y }, c);
                draw_vertex(MyVertex { x: min_x, y: max_y }, c);

                draw_line_p(min_x, min_y, max_x, min_y, 1.0, c);
                draw_line_p(max_x, min_y, max_x, max_y, 1.0, c);
                draw_line_p(max_x, max_y, min_x, max_y, 1.0, c);
                draw_line_p(min_x, max_y, min_x, min_y, 1.0, c);
            };

            /*
            for sector in &self.sectors {
            //let sector = &self.sectors[38]; {
                for line in &sector.lines {
                    draw_line(line.line, 1.0, [1.0, 0.0, 1.0, 1.0]);
                }
            }

            let mut index = 0;
            for sector in &self.sectors {
                let start = sector.box_start;
                let end = sector.box_end;

                /*
                draw_vertex(MyVertex { x: start.x, y: start.y }, COLOR_TABLE[index]);
                draw_vertex(MyVertex { x: end.x, y: start.y }, COLOR_TABLE[index]);
                draw_vertex(MyVertex { x: end.x, y: end.y }, COLOR_TABLE[index]);
                draw_vertex(MyVertex { x: start.x, y: end.y }, COLOR_TABLE[index]);
                */

                index += 1;
                if index >= COLOR_TABLE.len() {
                    index = 0;
                }
            }

            let node = self.gl_nodes[self.gl_nodes.len() - 1];
            // let node = self.gl_nodes[node.left_child];

            draw_line_p(node.x - node.dx * 40.0, node.y - node.dy * 40.0, node.x + node.dx * 40.0, node.y + node.dy * 40.0, 1.0, [0.0, 0.0, 1.0, 1.0]);
            draw_vertex(MyVertex { x: node.x, y: node.y }, [0.0, 1.0, 0.0, 1.0]);

            draw_box(node.left_box, [0.0, 1.0, 0.0, 1.0]);
            draw_box(node.right_box, [1.0, 0.0, 0.0, 1.0]);
            */

            // draw_line_p(x, y, x + w, y, 1.0, [0.0, 1.0, 1.0, 1.0]);
            // draw_line_p(x + w, y, x + w, y + h, 1.0, [0.0, 1.0, 1.0, 1.0]);
            // draw_line_p(x + w, y, x, y + h, 1.0, [0.0, 1.0, 1.0, 1.0]);
            // draw_line_p(x, y, x + w, y, 1.0, [0.0, 1.0, 1.0, 1.0]);

            /*
            // let sector = &self.sectors[29];
            for sector in &self.sectors {
                for segment in &sector.segments {
                    let vs_index = segment.start_vertex;
                    let ve_index = segment.end_vertex;

                    let vs = if vs_index & VERT_IS_GL == VERT_IS_GL {
                        self.gl_vertices[vs_index & !VERT_IS_GL]
                    } else {
                        self.vertices[vs_index]
                    };

                    let ve = if ve_index & VERT_IS_GL == VERT_IS_GL {
                        self.gl_vertices[ve_index & !VERT_IS_GL]
                    } else {
                        self.vertices[ve_index]
                    };

                    if segment.linedef != 0xffff {
                        let line = self.lines[segment.linedef];
                        draw_line(line.line, 0.5, [1.0, 0.0, 1.0, 1.0]);
                    } else {
                        draw_line_p(vs.x, vs.y, ve.x, ve.y, 1.0, [0.0, 0.0, 1.0, 1.0]);
                        draw_vertex(vs, [0.0, 0.0, 1.0, 1.0]);
                        draw_vertex(ve, [0.0, 0.0, 1.0, 1.0]);
                    }
                }
            }
            */

            let mut index = 0;
            let sector = &self.sectors[38]; {
            // for sector in &self.sectors {
                for sub_sectors in &sector.sub_sectors {
                    let mut verts = Vec::new();
                    for segment_index in 0..sub_sectors.count {
                        let segment = self.gl_segments[sub_sectors.start + segment_index];

                        let vs_index = segment.start_vertex;
                        let ve_index = segment.end_vertex;

                        if segment.linedef != 0xffff {
                            let line = self.lines[segment.linedef];
                            // draw_line(line.line, 0.5, [1.0, 0.0, 1.0, 1.0]);
                        }

                        let vs = if vs_index & VERT_IS_GL == VERT_IS_GL {
                            self.gl_vertices[vs_index & !VERT_IS_GL]
                        } else {
                            self.vertices[vs_index]
                        };

                        let ve = if ve_index & VERT_IS_GL == VERT_IS_GL {
                            self.gl_vertices[ve_index & !VERT_IS_GL]
                        } else {
                            self.vertices[ve_index]
                        };

                        verts.push(vs);
                        verts.push(ve);

                        // draw_line_p(vs.x, vs.y, ve.x, ve.y, 1.0, [0.0, 1.0, 0.0, 1.0]);
                    }

                    // verts.dedup();

                    let mut points = Vec::new();
                    for v in &verts {
                        // points.push(Point { x: v.x, y: v.y });
                        points.push([v.x, v.y]);
                    }

                    //let triangles = delaunator::triangulate(&points).triangles;
                    let triangles = triangulate(&verts).unwrap();
                    println!("Triangles: {}", triangles.len());

                    // polygon(COLOR_TABLE[index], &points, view, gl);

                    for i in 0..(triangles.len() / 3) {
                        let p1 = &triangles[i + 0];
                        let p2 = &triangles[i + 1];
                        let p3 = &triangles[i + 2];

                        draw_line_p(p1.x, p1.y, p2.x, p2.y, 1.0, [0.3, 1.0, 0.3, 1.0]);
                        draw_line_p(p2.x, p2.y, p3.x, p3.y, 1.0, [0.3, 1.0, 0.3, 1.0]);
                        draw_line_p(p3.x, p3.y, p1.x, p1.y, 1.0, [0.3, 1.0, 0.3, 1.0]);
                    }

                    index += 1;
                    if index >= COLOR_TABLE.len() {
                        index = 0;
                    }

                    for v in &verts {
                        draw_vertex(*v, [1.0, 0.0, 1.0, 1.0]);
                    }
                }
            }
            /*

            let node = self.gl_nodes[0];

            draw_line_p(node.x, node.y, node.x + node.dx, node.y + node.dx, 1.0, [0.0, 0.0, 1.0, 1.0]);

            let x = node.right_box.left;
            let y = node.right_box.top;
            let w = node.right_box.left - node.right_box.right;
            let h = node.right_box.top - node.right_box.right;

            draw_line_p(x, y, x + w, y, 1.0, [0.0, 1.0, 1.0, 1.0]);
            draw_line_p(x + w, y, x + w, y + h, 1.0, [0.0, 1.0, 1.0, 1.0]);
            // draw_line_p(x + w, y, x, y + h, 1.0, [0.0, 1.0, 1.0, 1.0]);
            // draw_line_p(x, y, x + w, y, 1.0, [0.0, 1.0, 1.0, 1.0]);

            */
            /*
            for sub_sector in &self.sub_sectors {
                for seg_index in 0..sub_sector.segment_count {
                    let segment = self.segments[sub_sector.start_segment + seg_index];

                    let vs_index = segment.start_vertex;
                    let ve_index = segment.end_vertex;

                    if segment.line_index != 0xffff {
                        let line = self.lines[segment.line_index];
                        // draw_line(line.line, 0.5, [1.0, 0.0, 1.0, 1.0]);
                    }

                    let vs = if vs_index & VERT_IS_GL == VERT_IS_GL {
                        self.gl_vertices[vs_index & !VERT_IS_GL]
                    } else {
                        self.vertices[vs_index]
                    };

                    let ve = if ve_index & VERT_IS_GL == VERT_IS_GL {
                        self.gl_vertices[ve_index & !VERT_IS_GL]
                    } else {
                        self.vertices[ve_index]
                    };

                    draw_vertex(vs, [1.0, 0.0, 1.0, 1.0]);
                    draw_vertex(ve, [1.0, 0.0, 1.0, 1.0]);
                    // draw_line_p(vs.x, vs.y, ve.x, ve.y, 1.0, [0.0, 1.0, 0.0, 1.0]);
                }
            }
            */

            /*
            for sub_sector in &self.gl_sub_sectors {
                for seg_index in 0..sub_sector.segment_count {
                    let segment = self.gl_segments[sub_sector.start_segment + seg_index];

                    let vs_index = segment.start_vertex;
                    let ve_index = segment.end_vertex;

                    if segment.line_index != 0xffff {
                        let line = self.lines[segment.line_index];
                        // draw_line(line.line, 0.5, [1.0, 0.0, 1.0, 1.0]);
                    }

                    let vs = if vs_index & VERT_IS_GL == VERT_IS_GL {
                        self.gl_vertices[vs_index & !VERT_IS_GL]
                    } else {
                        self.vertices[vs_index]
                    };

                    let ve = if ve_index & VERT_IS_GL == VERT_IS_GL {
                        self.gl_vertices[ve_index & !VERT_IS_GL]
                    } else {
                        self.vertices[ve_index]
                    };

                    draw_vertex(vs, [0.0, 0.0, 1.0, 1.0]);
                    draw_vertex(ve, [0.0, 0.0, 1.0, 1.0]);
                    draw_line_p(vs.x, vs.y, ve.x, ve.y, 1.0, [0.0, 1.0, 0.0, 1.0]);
                }
            }
            */

        });
    }

    fn update(&mut self, args: &UpdateArgs) {
        const CAMERA_SPEED: f64 = 1000.0;
        const ZOOM_SPEED: f64 = 2.0;

        if self.up {
            self.camera_y += CAMERA_SPEED * args.dt;
        }

        if self.down {
            self.camera_y -= CAMERA_SPEED * args.dt;
        }

        if self.left {
            self.camera_x += CAMERA_SPEED * args.dt;
        }

        if self.right {
            self.camera_x -= CAMERA_SPEED * args.dt;
        }

        if self.zoom_in {
            self.zoom -= ZOOM_SPEED * args.dt;
        }

        if self.zoom_out {
            self.zoom += ZOOM_SPEED * args.dt;
        }
    }
}

fn main() {
    // Change this to OpenGL::V2_1 if not working.
    let opengl = OpenGL::V3_2;

    // Create an Glutin window.
    let mut window: Window = WindowSettings::new("spinning-square", [1280, 720])
        .graphics_api(opengl)
        .exit_on_esc(true)
        .build()
        .unwrap();

    // Create a new game and run it.
    let mut app = App {
        gl: GlGraphics::new(opengl),
        camera_x: -1056.0,
        camera_y: 3616.0,
        zoom: 1.0,

        left: false,
        right: false,
        up: false,
        down: false,
        zoom_in: false,
        zoom_out: false,

        sub_sector_index: 0,
        vertices: Vec::new(),
        lines: Vec::new(),
        sidedefs: Vec::new(),
        sectors: Vec::new(),
        sub_sectors: Vec::new(),
        segments: Vec::new(),

        gl_vertices: Vec::new(),
        gl_segments: Vec::new(),
        gl_sub_sectors: Vec::new(),
        gl_nodes: Vec::new(),

        test_segments: Vec::new(),
    };

    // app.vertices.push(Vertex { x: 0.0, y: 0.0 });

    test_wad_data(&mut app);

    let mut events = Events::new(EventSettings::new());
    while let Some(e) = events.next(&mut window) {
        if let Some(args) = e.press_args() {
            if let Button::Keyboard(key) = args {
                match key {
                    Key::D => app.left = true,
                    Key::A => app.right = true,
                    Key::W => app.up = true,
                    Key::S => app.down = true,
                    Key::E => app.zoom_in = true,
                    Key::Q => app.zoom_out = true,
                    Key::R => { app.sub_sector_index += 1; println!("Index: {}", app.sub_sector_index);},
                    Key::T => app.sub_sector_index -= 1,
                    _ => {}
                }
            }
        }

        if let Some(args) = e.release_args() {
            if let Button::Keyboard(key) = args {
                match key {
                    Key::D => app.left = false,
                    Key::A => app.right = false,
                    Key::W => app.up = false,
                    Key::S => app.down = false,
                    Key::E => app.zoom_in = false,
                    Key::Q => app.zoom_out = false,
                    _ => {}
                }
            }
        }

        if let Some(args) = e.render_args() {
            app.render(&args);
        }

        if let Some(args) = e.update_args() {
            app.update(&args);
        }
    }
}
