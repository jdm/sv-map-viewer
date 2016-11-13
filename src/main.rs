extern crate graphics;
extern crate glutin_window;
extern crate image;
extern crate opengl_graphics;
extern crate piston;
extern crate xnb;

use glutin_window::GlutinWindow as Window;
use graphics::Image;
use image::RgbaImage;
use opengl_graphics::{GlGraphics, OpenGL, Texture, TextureSettings, Filter, ImageSize};
use piston::window::WindowSettings;
use piston::event_loop::*;
use piston::input::*;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::path::Path;
use xnb::{XNB, Asset};
use xnb::tide::{TileSheet, Layer};

const SCALE: f64 = 2.0;

pub struct App {
    gl: GlGraphics,
    view_x: u32,
    view_y: u32,
}

struct Tile<'a> {
    sheet: &'a TileSheet,
    index: u32,
}

fn image_for_tile(tile: &Tile, pos: (u32, u32), view: (u32, u32)) -> Image {
    let num_h_tiles = tile.sheet.sheet_size.0;
    let tile_w = tile.sheet.tile_size.0;
    let tile_h = tile.sheet.tile_size.1;
    image_for_tile_reference(num_h_tiles,
                             (tile_w, tile_h),
                             tile.index,
                             pos,
                             (0, 0),
                             view)
}

fn image_for_texture(texture: &TextureTileInfo,
                     pos: (u32, u32),
                     view: (u32, u32)) -> Image {
    let num_h_tiles = texture.0.get_width() / 16;
    image_for_tile_reference(num_h_tiles,
                             texture.2.clone(),
                             texture.1,
                             pos,
                             texture.3.clone(),
                             view)
}

fn image_for_tile_reference(num_h_tiles: u32,
                            (tile_w, tile_h): (u32, u32),
                            index: u32,
                            (x, y): (u32, u32),
                            (off_x, off_y): (i32, i32),
                            (view_x, view_y): (u32, u32)) -> Image {
    let src_x = index % num_h_tiles * tile_w;
    let src_y = index / num_h_tiles * tile_h;
    Image::new()
        .src_rect([src_x as i32,
                   src_y as i32,
                   tile_w as i32,
                   tile_h as i32])
        .rect([((x - view_x) * 16) as f64 + off_x as f64,
               ((y - view_y) * 16) as f64 + off_y as f64,
               tile_w as f64,
               tile_h as f64])
}

type TextureTileInfo = (Texture, u32, (u32, u32), (i32, i32));

struct Player {
    base: TextureTileInfo,
    bottom: TextureTileInfo,
    arms: TextureTileInfo,
    pants: TextureTileInfo,
    hairstyle: TextureTileInfo,
    hat: TextureTileInfo,
    shirt: TextureTileInfo,
    accessory: TextureTileInfo,
}

impl App {
    fn render(&mut self,
              args: &RenderArgs,
              textures: &HashMap<String, Texture>,
              tilesheets: &[TileSheet],
              player: &Player,
              layers: &[Layer]) {
        use graphics::*;

        const BLACK: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

        let view_x = self.view_x;
        let view_y = self.view_y;
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
                    let (x, y) = base_tile.get_pos();
                    if x < view_x || y < view_y {
                        continue;
                    }
                    let image = image_for_tile(&tile, (x, y), (view_x, view_y));
                    let transform = c.transform.zoom(SCALE);
                    image.draw(texture, &Default::default(), transform, gl);
                }
            }

            let view = (view_x, view_y);
            let pos = (5, 5);

            let transform = c.transform.zoom(SCALE);

            // Body
            let image = image_for_texture(&player.base, pos, view);
            image.draw(&player.base.0, &Default::default(), transform, gl);
            let image = image_for_texture(&player.bottom, pos, view);
            image.draw(&player.bottom.0, &Default::default(), transform, gl);

            // Hair
            let image = image_for_texture(&player.hairstyle, pos, view);
            image.draw(&player.hairstyle.0, &Default::default(), transform, gl);

            // Hat
            let image = image_for_texture(&player.hat, pos, view);
            image.draw(&player.hat.0, &Default::default(), transform, gl);

            // Arms
            let image = image_for_texture(&player.arms, pos, view);
            image.draw(&player.arms.0, &Default::default(), transform, gl);

            // Pants
            let image = image_for_texture(&player.pants, pos, view);
            image.draw(&player.pants.0, &Default::default(), transform, gl);

            // Shirt
            let image = image_for_texture(&player.shirt, pos, view);
            image.draw(&player.shirt.0, &Default::default(), transform, gl);

            // Facial accessory
            let image = image_for_texture(&player.accessory, pos, view);
            image.draw(&player.accessory.0, &Default::default(), transform, gl);
        });
    }

    fn update(&mut self, _args: &UpdateArgs) {
        // Rotate 2 radians per second.
        //self.rotation += 2.0 * args.dt;
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
    let map = args.next().unwrap_or("Town.xnb".into());

    let base = Path::new("../xnb/uncompressed/Maps");
    let f = File::open(base.join(map)).unwrap();
    let xnb = XNB::from_buffer(f).unwrap();
    let map = match xnb.primary {
        Asset::Tide(map) => map,
        _ => panic!("unexpected xnb contents"),
    };

    fn load_texture(base: &Path, filename: &str) -> Texture {
        let f = File::open(base.join(filename)).unwrap();
        let xnb = XNB::from_buffer(f).unwrap();
        match xnb.primary {
            Asset::Texture2d(mut texture) => {
                let img = RgbaImage::from_raw(texture.width as u32,
                                              texture.height as u32,
                                              texture.mip_data.remove(0)).unwrap();
                let mut settings = TextureSettings::new();
                settings.set_filter(Filter::Nearest);
                Texture::from_image(&img, &settings)
            }
            _ => panic!("unexpected xnb contents"),
        }
    }

    let mut tilesheets = HashMap::new();
    for ts in &map.tilesheets {
        let texture = load_texture(base, &format!("{}.xnb", ts.image_source));
        println!("storing texture for {}", ts.id);
        tilesheets.insert(ts.id.clone(), texture);
    }
    println!("loaded {} tilesheets", tilesheets.len());

    let path = Path::new("../xnb/uncompressed/Characters/Farmer");
    let base = load_texture(path, "farmer_base.xnb");
    let bottom = load_texture(path, "farmer_base.xnb");
    let arms = load_texture(path, "farmer_base.xnb");
    let pants = load_texture(path, "farmer_base.xnb");
    let hairstyle = load_texture(path, "hairstyles.xnb");
    let hat = load_texture(path, "hats.xnb");
    let shirt = load_texture(path, "shirts.xnb");
    let accessory = load_texture(path, "accessories.xnb");
    let player = Player {
        base: (base, 0, (16, 16), (0, 0)),
        bottom: (bottom, 24, (16, 16), (0, 16)),
        arms: (arms, 30, (16, 16), (0, 16)),
        hairstyle: (hairstyle, 0, (16, 16), (0, 0)),
        hat: (hat, 2, (20, 20), (-2, -2)),
        pants: (pants, 42, (16, 16), (0, 16)),
        shirt: (shirt, 0, (8, 8), (4, 15)),
        accessory: (accessory, 0, (16, 16), (0, 3)),
    };

    // Create a new game and run it.
    let mut app = App {
        gl: GlGraphics::new(opengl),
        view_x: 0,
        view_y: 0,
    };

    let mut events = window.events();
    while let Some(e) = events.next(&mut window) {
        if let Some(Button::Keyboard(k)) = e.press_args() {
            match k {
                Key::Left if app.view_x > 0 => app.view_x -= 1,
                Key::Right => app.view_x += 1,
                Key::Up if app.view_y > 0 => app.view_y -= 1,
                Key::Down => app.view_y += 1,
                _ => {}
            }
        }

        if let Some(r) = e.render_args() {
            app.render(&r, &tilesheets, &map.tilesheets, &player, &map.layers);
        }

        if let Some(u) = e.update_args() {
            app.update(&u);
        }
    }
}
