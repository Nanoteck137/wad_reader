use std::path::Path;
use std::fs::File;
use std::io::Read;

use glutin_window::GlutinWindow as Window;
use opengl_graphics::{GlGraphics, OpenGL};
use piston::event_loop::{EventSettings, Events};
use piston::input::{RenderArgs, RenderEvent, UpdateArgs, UpdateEvent, PressEvent, ReleaseEvent, Key, Button};
use piston::window::WindowSettings;

#[derive(Copy, Clone, Debug)]
struct MyVertex {
    x: f32,
    y: f32,
}

#[derive(Copy, Clone, Debug)]
struct MyLine {
    start_vertex: usize,
    end_vertex: usize,
}

#[derive(Copy, Clone, Debug)]
struct MySegments {
    line: MyLine,
    line_index: usize,
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

    vertices: Vec<MyVertex>,
    lines: Vec<MyLine>,
    segments: Vec<MySegments>
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

            app.lines.push(MyLine {
                start_vertex: start_vertex.try_into().unwrap(),
                end_vertex: end_vertex.try_into().unwrap(),
            });
        }
    }

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

            app.segments.push(MySegments {
                line: MyLine {
                    start_vertex: start_vertex.try_into().unwrap(),
                    end_vertex: end_vertex.try_into().unwrap(),
                },
                line_index: line_index.try_into().unwrap(),
            });
        }
    }

    /*
    let data_offset: usize = data_offset.try_into().unwrap();
    for i in 0..num_dirs {
        let start = data_offset + i * 16;
        let bytes = &data[start..start+16];

        let lump_offset = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let lump_size = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let lump_name = &bytes[8..16];
        println!("Name: {:?} {:#x} {}",
                 std::str::from_utf8(&lump_name),
                 lump_offset, lump_size);
    }
    */
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
                let start = self.vertices[l.start_vertex];
                let end = self.vertices[l.end_vertex];
                line_from_to(c, s, [start.x as f64, start.y as f64], [end.x as f64, end.y as f64], view, unsafe { *ptr });
            };

            let mut draw_vertex = |v: MyVertex, c| {
                let x: f64 = v.x.into();
                let y: f64 = v.y.into();
                let transform = identity().trans(x - 5.0, y - 5.0);

                ellipse(c, square, view.append_transform(transform), gl);
            };

            for vert in &self.vertices {
                // draw_vertex(*vert, RED);
            }

            // polygon([1.0, 0.0, 1.0, 1.0], &verts, view, gl);

            for l in &self.lines {
                // draw_line(*l, 2.0);
            }

            for s in &self.segments {
                let start_vert = self.vertices[s.line.start_vertex];
                let end_vert = self.vertices[s.line.end_vertex];
                let line = self.lines[s.line_index];

                draw_vertex(start_vert, [1.0, 0.0, 0.0, 1.0]);
                draw_vertex(end_vert, [0.0, 1.0, 0.0, 1.0]);
                draw_line(line, 1.0, [0.3, 0.3, 1.0, 1.0]);
            }

            // polygon([1.0, 0.0, 1.0, 1.0], &[[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]], c.transform, gl);
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

        vertices: Vec::new(),
        lines: Vec::new(),
        segments: Vec::new(),
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
