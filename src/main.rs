extern crate graphics;
extern crate glutin_window;
extern crate image;
extern crate opengl_graphics;
extern crate piston;
extern crate xnb;

use glutin_window::GlutinWindow as Window;
use graphics::Image;
use image::RgbaImage;
use opengl_graphics::{GlGraphics, OpenGL, Texture, TextureSettings};
use piston::window::WindowSettings;
use piston::event_loop::*;
use piston::input::*;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::path::Path;
use xnb::{XNB, Asset};
use xnb::tide::{TileSheet, Layer};

pub struct App {
    gl: GlGraphics,
    rotation: f64,
}

struct Tile<'a> {
    sheet: &'a TileSheet,
    index: u32,
}

fn image_for_tile(tile: &Tile) -> Image {
    let num_h_tiles = tile.sheet.sheet_size.0;
    let tile_w = tile.sheet.tile_size.0;
    let tile_h = tile.sheet.tile_size.1;
    let src_x = tile.index % num_h_tiles * tile_w;
    let src_y = tile.index / num_h_tiles * tile_h;
    Image::new()
        .src_rect([src_x as i32,
                   src_y as i32,
                   tile_w as i32,
                   tile_h as i32])
}

impl App {
    fn render(&mut self,
              args: &RenderArgs,
              textures: &HashMap<String, Texture>,
              tilesheets: &[TileSheet],
              layers: &[Layer]) {
        use graphics::*;

        const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

        self.gl.draw(args.viewport(), |c, gl| {
            // Clear the screen.
            clear(BLACK, gl);

            for layer in layers.iter() {
                if !layer.visible {
                    continue;
                }
                for base_tile in layer.tiles.iter() {
                    let tilesheet_name = base_tile.get_tilesheet();
                    let texture = textures.get(tilesheet_name).expect("no texture");
                    let tilesheet = tilesheets.iter().find(|s| s.id == tilesheet_name).expect("no tilesheet");
                    let tile = Tile {
                        sheet: tilesheet,
                        index: base_tile.get_index(),
                    };
                    let (mut x, mut y) = base_tile.get_pos();
                    x *= tilesheet.tile_size.0;
                    y *= tilesheet.tile_size.1;
                    let image = image_for_tile(&tile);
                    let transform = c.transform.trans(x as f64, y as f64);
                    image.draw(texture, &Default::default(), transform, gl);
                }
            }
        });
    }

    fn update(&mut self, args: &UpdateArgs) {
        // Rotate 2 radians per second.
        self.rotation += 2.0 * args.dt;
    }
}

fn main() {
    // Change this to OpenGL::V2_1 if not working.
    let opengl = OpenGL::V3_2;

    // Create an Glutin window.
    let mut window: Window = WindowSettings::new(
            "spinning-square",
            [800, 600]
        )
        .opengl(opengl)
        .exit_on_esc(true)
        .build()
        .unwrap();

    let mut args = env::args();
    let _self = args.next();
    let map = args.next().unwrap();

    let base = Path::new("../xnb/data/uncompressed");
    let f = File::open(base.join(map)).unwrap();
    let xnb = XNB::from_buffer(f).unwrap();
    let map = match xnb.primary {
        Asset::Tide(map) => map,
        _ => panic!("unexpected xnb contents"),
    };

    let mut tilesheets = HashMap::new();
    for ts in &map.tilesheets {
        let f = File::open(base.join(format!("{}.xnb", ts.image_source))).unwrap();
        let xnb = XNB::from_buffer(f).unwrap();
        match xnb.primary {
            Asset::Texture2d(mut texture) => {
                let img = RgbaImage::from_raw(texture.width as u32,
                                              texture.height as u32,
                                              texture.mip_data.remove(0)).unwrap();
                let texture = Texture::from_image(&img, &TextureSettings::new());
                println!("storing texture for {}", ts.id);
                tilesheets.insert(ts.id.clone(), texture);
            }
            _ => panic!("unexpected xnb contents"),
        };
    }
    println!("loaded {} tilesheets", tilesheets.len());

    // Create a new game and run it.
    let mut app = App {
        gl: GlGraphics::new(opengl),
        rotation: 0.0,
    };

    let mut events = window.events();
    while let Some(e) = events.next(&mut window) {
        if let Some(r) = e.render_args() {
            app.render(&r, &tilesheets, &map.tilesheets, &map.layers);
        }

        if let Some(u) = e.update_args() {
            app.update(&u);
        }
    }
}
