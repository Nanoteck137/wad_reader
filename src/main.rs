use std::path::Path;
use std::fs::File;
use std::io::Read;

use glutin_window::GlutinWindow as Window;
use opengl_graphics::{GlGraphics, OpenGL};
use piston::event_loop::{EventSettings, Events};
use piston::input::{RenderArgs, RenderEvent, UpdateArgs, UpdateEvent, PressEvent, ReleaseEvent, Key, Button};
use piston::window::WindowSettings;

use wad::Wad;

mod wad;
mod mime;

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

struct App {
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
}

fn read_file<P>(path: P) -> Vec<u8>
    where P: AsRef<Path>
{
    let mut file = File::open(path).unwrap();

    let mut result = Vec::new();
    file.read_to_end(&mut result).unwrap();

    result
}


fn load_wad_map_data() -> wad::Map {
    // Read the raw wad file
    let data = read_file("doom1.wad");
    // Parse the wad
    let wad = Wad::parse(&data)
        .expect("Failed to parse WAD file");

    // Construct an map with map from the wad
    let map = wad::Map::parse_from_wad(&wad, "E1M1")
        .expect("Failed to load map E1M1");

    // Generate the vertices and indices for the map
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let mut index = 0;
    for sector in &map.sectors {
        for sub_sector in &sector.sub_sectors {
            let mut seg_verts = Vec::new();
            for segment in 0..sub_sector.count {
                let segment = map.segments[sub_sector.start + segment];
                let start = map.vertex(segment.start_vertex);

                let color = COLOR_TABLE[index];
                seg_verts.push(mime::Vertex::new(start.x, start.y, color));
            }

            cleanup_lines(&mut seg_verts);
            let triangles = triangulate(&seg_verts).unwrap();

            let index_offset = vertices.len();

            for v in &seg_verts {
                vertices.push(*v);
            }

            for i in &triangles {
                indices.push(i + index_offset as u32);
            }

            index += 1;
            if index >= COLOR_TABLE.len() {
                index = 0;
            }
        }
    }

    let mime_map = mime::Map::new(vertices, indices);
    mime_map.save_to_file("map.mup").unwrap();


    map
}

fn triangulate(polygon: &Vec<mime::Vertex>) -> Option<Vec<u32>> {
    let mut indices = Vec::new();

    let p0 = 0u32;
    let mut p1 = 1u32;

    let mut index = 2;

    loop {
        if index >= polygon.len() {
            break;
        }

        indices.push(p0);

        let p2 = index as u32;

        indices.push(p1);
        indices.push(p2);

        p1 = p2;

        index += 1;
    }

    Some(indices)
}

fn line_angle(a: mime::Vertex, b: mime::Vertex) -> f32 {
    (b.y - a.y).atan2(b.x - a.x)
}

fn point_on_line(a: mime::Vertex, b: mime::Vertex, c: mime::Vertex) -> bool {
    return (line_angle(a, b) - line_angle(b, c)).abs() < 0.05
}

fn cleanup_lines(verts: &mut Vec<mime::Vertex>) {
    for i in 0..verts.len() {
        let p1 = verts[i % verts.len()];
        let p2 = verts[i.wrapping_add(1) % verts.len()];
        let p3 = verts[i.wrapping_add(2) % verts.len()];

        if point_on_line(p1, p2, p3) {
            verts.remove((i.wrapping_add(1) as usize) % verts.len());
        }
    }
}

impl App {
    fn render(&mut self, args: &RenderArgs) {
        use graphics::*;

        let viewport = args.viewport();

        self.gl.draw(viewport, |c, mut gl| {
            // Clear the screen.
            clear([0.0, 0.0, 0.0, 1.0], gl);

            let ptr = std::ptr::addr_of_mut!(gl);

            let view = c.view.trans(-self.camera_x, self.camera_y).zoom(self.zoom);

            /*
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
                line_from_to(c, s, [x1 as f64, y1 as f64], [x2 as f64, y2 as f64], view, unsafe { *ptr });
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

                /*
                draw_vertex(MyVertex { x: min_x, y: min_y }, c);
                draw_vertex(MyVertex { x: max_x, y: min_y }, c);
                draw_vertex(MyVertex { x: max_x, y: max_y }, c);
                draw_vertex(MyVertex { x: min_x, y: max_y }, c);
                */

                draw_line_p(min_x, min_y, max_x, min_y, 1.0, c);
                draw_line_p(max_x, min_y, max_x, max_y, 1.0, c);
                draw_line_p(max_x, max_y, min_x, max_y, 1.0, c);
                draw_line_p(min_x, max_y, min_x, min_y, 1.0, c);
            };
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
    let mut window: Window = WindowSettings::new("wad-reader", [1280, 720])
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
    };

    let map = load_wad_map_data();

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
