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

/// TODO(patrik):
///   - Parse texture data
///   - Map format
///     - Seperate vertex and index buffers for the different sectors
///       of the map
///     - Textures

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

    map: wad::Map,
}

fn read_file<P>(path: P) -> Vec<u8>
    where P: AsRef<Path>
{
    let mut file = File::open(path).unwrap();

    let mut result = Vec::new();
    file.read_to_end(&mut result).unwrap();

    result
}

fn generate_sector_from_wad(map: &wad::Map,
                            sector: &wad::Sector)
    -> Option<mime::Sector>
{
    let mut vertex_buffer = Vec::new();
    let mut index_buffer = Vec::new();

    let mut add_vertices = |mut verts, clockwise, cleanup| {
        if cleanup {
            cleanup_lines(&mut verts);
        }

        let triangles = triangulate(&vertex_buffer, clockwise).unwrap();

        let index_offset = vertex_buffer.len();

        for v in &verts {
            vertex_buffer.push(*v);
        }

        for i in &triangles {
            index_buffer.push(i + index_offset as u32);
        }
    };

    let mut index = 0;
    for sub_sector in &sector.sub_sectors {
        let mut floor = Vec::new();
        let mut ceiling = Vec::new();
        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];
            let start = map.vertex(segment.start_vertex);

            let color = COLOR_TABLE[index];
            floor.push(mime::Vertex::new(start.x, sector.floor_height, start.y, color));
            ceiling.push(mime::Vertex::new(start.x, sector.ceiling_height, start.y, color));

            if segment.linedef != 0xffff {
                let mut wall = Vec::new();
                let linedef = map.linedefs[segment.linedef];
                let line = linedef.line;
                let start = map.vertex(line.start_vertex);
                let end = map.vertex(line.end_vertex);

                if linedef.flags & wad::LINEDEF_FLAG_IMPASSABLE == wad::LINEDEF_FLAG_IMPASSABLE &&
                    linedef.flags & wad::LINEDEF_FLAG_TWO_SIDED != wad::LINEDEF_FLAG_TWO_SIDED
                {
                    wall.push(mime::Vertex::new(start.x, sector.floor_height, start.y, color));
                    wall.push(mime::Vertex::new(end.x, sector.floor_height, end.y, color));
                    wall.push(mime::Vertex::new(end.x, sector.ceiling_height, end.y, color));
                    wall.push(mime::Vertex::new(start.x, sector.ceiling_height, start.y, color));
                }

                add_vertices(wall, false, false);
            }
        }

        index += 1;
        if index >= COLOR_TABLE.len() {
            index = 0;
        }

        add_vertices(floor, true, true);
        add_vertices(ceiling, false, true);
    }

    let mut index = 0;

    for sub_sector in &sector.sub_sectors {
        for segment in 0..sub_sector.count {
            let segment = map.segments[sub_sector.start + segment];

            if segment.linedef != 0xffff {
                let linedef = map.linedefs[segment.linedef];
                let line = linedef.line;

                let start = map.vertex(line.start_vertex);
                let end = map.vertex(line.end_vertex);

                if linedef.front_sidedef.is_some() && linedef.back_sidedef.is_some() {
                    let front_sidedef = linedef.front_sidedef.unwrap();
                    let front_sidedef = map.sidedefs[front_sidedef];

                    let back_sidedef = linedef.back_sidedef.unwrap();
                    let back_sidedef = map.sidedefs[back_sidedef];

                    let front_sector = &map.sectors[front_sidedef.sector];
                    let back_sector = &map.sectors[back_sidedef.sector];

                    // Generate the floor difference
                    if front_sector.floor_height != back_sector.floor_height {
                        // TODO(patrik): Generate the vertices

                        let front = front_sector.floor_height;
                        let back = back_sector.floor_height;

                        let mut verts = Vec::new();

                        let color = COLOR_TABLE[index]; //[1.0, 0.0, 1.0, 1.0];
                        verts.push(mime::Vertex::new(start.x, front, start.y, color));
                        verts.push(mime::Vertex::new(end.x, front, end.y, color));
                        verts.push(mime::Vertex::new(end.x, back, end.y, color));
                        verts.push(mime::Vertex::new(start.x, back, start.y, color));

                        add_vertices(verts, false, false);
                        index += 1;
                        if index >= COLOR_TABLE.len() {
                            index = 0;
                        }
                    }

                    // Generate the height difference
                    if front_sector.ceiling_height != back_sector.ceiling_height {
                        // TODO(patrik): Generate the vertices

                        let front = front_sector.ceiling_height;
                        let back = back_sector.ceiling_height;

                        let mut verts = Vec::new();

                        let color = COLOR_TABLE[index]; // [1.0, 0.0, 1.0, 1.0];
                        verts.push(mime::Vertex::new(start.x, front, start.y, color));
                        verts.push(mime::Vertex::new(end.x, front, end.y, color));
                        verts.push(mime::Vertex::new(end.x, back, end.y, color));
                        verts.push(mime::Vertex::new(start.x, back, start.y, color));

                        add_vertices(verts, true, false);

                        index += 1;
                        if index >= COLOR_TABLE.len() {
                            index = 0;
                        }
                    }
                }
            }
        }
    }

    Some(mime::Sector::new(vertex_buffer, index_buffer))
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

    let mut sectors = Vec::new();

    for sector in &map.sectors {
        let map_sector = generate_sector_from_wad(&map, sector).unwrap();
        sectors.push(map_sector);
    }

    let mime_map = mime::Map::new(sectors);
    mime_map.save_to_file("map.mup").unwrap();

    map
}

fn triangulate(polygon: &Vec<mime::Vertex>, clockwise: bool) -> Option<Vec<u32>> {
    let mut indices = Vec::new();

    let p0 = 0u32;
    let mut p1 = 1u32;

    let mut index = 2;

    loop {
        if index >= polygon.len() {
            break;
        }

        let p2 = index as u32;

        if clockwise {
            indices.push(p0);
            indices.push(p1);
            indices.push(p2);
        } else {
            indices.push(p0);
            indices.push(p2);
            indices.push(p1);
        }

        p1 = p2;

        index += 1;
    }

    Some(indices)
}

fn line_angle(a: mime::Vertex, b: mime::Vertex) -> f32 {
    (b.z - a.z).atan2(b.x - a.x)
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
            verts.remove(i.wrapping_add(1) % verts.len());
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

            let mut draw_line = |l: wad::Line, s, c| {
                let start = self.map.vertex(l.start_vertex);
                let end = self.map.vertex(l.end_vertex);

                line_from_to(c, s, [start.x as f64, start.y as f64], [end.x as f64, end.y as f64], view, unsafe { *ptr });
            };

            /*
            let mut draw_line_p = |x1, y1, x2, y2, s, c| {
                line_from_to(c, s, [x1 as f64, y1 as f64], [x2 as f64, y2 as f64], view, unsafe { *ptr });
            };

            let mut draw_vertex = |v: MyVertex, c| {
                let x: f64 = v.x.into();
                let y: f64 = v.y.into();
                let transform = identity().trans(x - 5.0, y - 5.0);

                ellipse(c, square, view.append_transform(transform), unsafe { *ptr });
            };
            */

            /*

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

        let sector = &self.map.sectors[39]; {
            //let sub_sector = sector.sub_sectors[1]; {
            for sub_sector in &sector.sub_sectors {
                //let segment = 0; {
                for segment in 0..sub_sector.count {
                    let segment = self.map.segments[sub_sector.start + segment];

                    if segment.linedef != 0xffff {
                        let linedef = self.map.linedefs[segment.linedef];
                        draw_line(linedef.line, 1.0, [1.0, 0.0, 1.0, 1.0]);
                    }
                }
            }
        }

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

    let map = load_wad_map_data();

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

        map,
    };

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
