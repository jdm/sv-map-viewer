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
use xnb::{XNB, Asset, DictionaryKey};
use xnb::tide::{TileSheet, Layer};

const SCALE: f64 = 1.5;

pub struct App {
    gl: GlGraphics,
    view_x: u32,
    view_y: u32,
    ticks: u32,
    d_pressed: bool,
    a_pressed: bool,
    w_pressed: bool,
    s_pressed: bool,
    update_last_move: bool,
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
                             0,
                             pos,
                             (0, 0),
                             view,
                             false)
}

#[derive(Copy, Clone, PartialEq)]
enum PlayerDir {
    Down = 0,
    Right = 1,
    Up = 2,
    Left = 3,
}

fn image_for_texture(texture: &TextureTileInfo,
                     pos: (u32, u32),
                     view: (u32, u32),
                     offset: (i32, i32),
                     anim: Option<(u32, u32)>,
                     dir: PlayerDir) -> Image {
    let num_h_tiles = texture.0.get_width() / (texture.2).0;
    let offset = ((texture.3).0 + offset.0, (texture.3).1 + offset.1);
    let base = texture.4[dir as usize].unwrap_or(0);
    let flip = dir == PlayerDir::Left && texture.4[PlayerDir::Left as usize] == texture.4[PlayerDir::Right as usize];
    let anim_idx = anim.map_or(0, |a| a.0 / 150 % a.1);
    image_for_tile_reference(num_h_tiles,
                             texture.2.clone(),
                             texture.1 + anim_idx,
                             base,
                             pos,
                             offset,
                             view,
                             flip)
}

fn image_for_tile_reference(num_h_tiles: u32,
                            (tile_w, tile_h): (u32, u32),
                            index: u32,
                            index_y_offset: u32,
                            (x, y): (u32, u32),
                            (off_x, off_y): (i32, i32),
                            (view_x, view_y): (u32, u32),
                            flip_h: bool) -> Image {
    let src_x = index % num_h_tiles * tile_w;
    let src_y = (index / num_h_tiles + index_y_offset) * tile_h;
    let src_rect = if flip_h {
        [src_x as i32 + tile_w as i32,
         src_y as i32,
         -(tile_w as i32),
         tile_h as i32]
    } else {
        [src_x as i32, src_y as i32, tile_w as i32, tile_h as i32]
    };
    Image::new()
        .src_rect(src_rect)
        .rect([((x - view_x) * 16) as f64 + off_x as f64,
               ((y - view_y) * 16) as f64 + off_y as f64,
               tile_w as f64,
               tile_h as f64])
}

type TextureTileInfo = (Texture, u32, (u32, u32), (i32, i32), [Option<u32>; 4]);

struct Player {
    base: TextureTileInfo,
    bottom: TextureTileInfo,
    arms: TextureTileInfo,
    pants: TextureTileInfo,
    hairstyle: TextureTileInfo,
    hat: TextureTileInfo,
    shirt: TextureTileInfo,
    accessory: TextureTileInfo,
    x: u32,
    y: u32,
    offset_x: f64,
    offset_y: f64,
    last_move_start: Option<u32>,
    dir: PlayerDir,
}

impl Player {
    fn move_horiz(&mut self, delta: f64) {
        self.offset_x += delta;
        if delta < 0. && self.offset_x < -8. {
            self.offset_x = 8.;
            self.x -= 1;
        } else if delta > 0. && self.offset_x > 8. {
            self.offset_x = -8.;
            self.x += 1;
        }
    }

    fn move_vert(&mut self, delta: f64) {
        self.offset_y += delta;
        if delta < 0. && self.offset_y < -8. {
            self.offset_y = 8.;
            self.y -= 1;
        } else if delta > 0. && self.offset_y > 8. {
            self.offset_y = -8.;
            self.y += 1;
        }
    }
}


impl App {
    fn render(&mut self,
              args: &RenderArgs,
              textures: &HashMap<String, Texture>,
              tilesheets: &[TileSheet],
              player: &Player,
              layers: &[Layer]) {
        use graphics::*;

        const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

        let view_x = self.view_x;
        let view_y = self.view_y;

        let view_w = args.viewport().window_size[0] / 16 + view_x;
        let view_h = args.viewport().window_size[1] / 16 + view_y;

        let ticks = self.ticks;

        fn draw_layer(layer: &Layer,
                      textures: &HashMap<String, Texture>,
                      tilesheets: &[TileSheet],
                      transform: [[f64; 3]; 2],
                      gl: &mut GlGraphics,
                      ticks: u32,
                      (view_x, view_y): (u32, u32),
                      (view_w, view_h): (u32, u32)) {
            if !layer.visible  || layer.id == "Paths" {
                return;
            }
            for base_tile in layer.tiles.iter() {
                let tilesheet_name = base_tile.get_tilesheet();
                let texture = textures.get(tilesheet_name).expect("no texture");
                let tilesheet = tilesheets.iter().find(|s| s.id == tilesheet_name).expect("no tilesheet");
                let tile = Tile {
                    sheet: tilesheet,
                    index: base_tile.get_index(ticks),
                };
                let (x, y) = base_tile.get_pos();
                if x < view_x || x > view_w || y < view_y || y > view_h {
                    continue;
                }
                let image = image_for_tile(&tile, (x, y), (view_x, view_y));
                image.draw(texture, &Default::default(), transform, gl);
            }
        }

        self.gl.draw(args.viewport(), |c, gl| {
            // Clear the screen.
            clear(BLACK, gl);

            let view = (view_x, view_y);
            let transform = c.transform.zoom(SCALE);

            for (i, layer) in layers.iter().enumerate() {
                if i == layers.len() - 1 {
                    break;
                }
                draw_layer(layer, textures, tilesheets, transform, gl, ticks,
                           (view_x, view_y), (view_w, view_h));
            }

            let pos = (player.x, player.y);
            let offset = (player.offset_x as i32, player.offset_y as i32);

            let transform = c.transform.clone().zoom(SCALE);

            let player_ticks = match player.last_move_start {
                Some(start) => ticks - start,
                None => 0,
            };

            let three_frame = Some((player_ticks, 3));

            // Body
            let image = image_for_texture(&player.base, pos, view, offset, three_frame, player.dir);
            image.draw(&player.base.0, &Default::default(), transform, gl);
            let image = image_for_texture(&player.bottom, pos, view, offset, three_frame, player.dir);
            image.draw(&player.bottom.0, &Default::default(), transform, gl);

            // Hair
            let image = image_for_texture(&player.hairstyle, pos, view, offset, None, player.dir);
            image.draw(&player.hairstyle.0, &Default::default(), transform, gl);

            // Hat
            let image = image_for_texture(&player.hat, pos, view, offset, None, player.dir);
            image.draw(&player.hat.0, &Default::default(), transform, gl);

            // Arms
            let image = image_for_texture(&player.arms, pos, view, offset, three_frame, player.dir);
            image.draw(&player.arms.0, &Default::default(), transform, gl);

            // Pants
            let image = image_for_texture(&player.pants, pos, view, offset, three_frame, player.dir);
            image.draw(&player.pants.0, &Default::default(), transform, gl);

            // Shirt
            let image = image_for_texture(&player.shirt, pos, view, offset, None, player.dir);
            image.draw(&player.shirt.0, &Default::default(), transform, gl);

            // Facial accessory
            if player.dir != PlayerDir::Up {
                let image = image_for_texture(&player.accessory, pos, view, offset, None, player.dir);
                image.draw(&player.accessory.0, &Default::default(), transform, gl);
            }

            draw_layer(layers.last().unwrap(), textures, tilesheets, transform, gl, ticks,
                       (view_x, view_y), (view_w, view_h));
        });
    }

    fn key_released(&mut self, key: Key) {
        match key {
            Key::A => self.a_pressed = false,
            Key::D => self.d_pressed = false,
            Key::W => self.w_pressed = false,
            Key::S => self.s_pressed = false,
            _ => {}
        }

        self.update_last_move = true;
    }

    fn key_pressed(&mut self, key: Key) {
        if key == Key::W && !self.w_pressed ||
            key == Key::S && !self.s_pressed ||
            key == Key::A && !self.a_pressed ||
            key == Key::D && !self.d_pressed {
            self.update_last_move = true
        }

        match key {
            Key::A => self.a_pressed = true,
            Key::D => self.d_pressed = true,
            Key::S => self.s_pressed = true,
            Key::W => self.w_pressed = true,
            _ => {}
        }
    }

    fn update(&mut self, args: &UpdateArgs, player: &mut Player) {
        self.ticks += (args.dt * 1000.) as u32;

        if self.update_last_move {
            self.update_last_move = false;
            if self.a_pressed || self.d_pressed || self.s_pressed || self.w_pressed {
                if self.w_pressed {
                    player.dir = PlayerDir::Up;
                }
                if self.s_pressed {
                    player.dir = PlayerDir::Down;
                }
                if self.a_pressed {
                    player.dir = PlayerDir::Left;
                }
                if self.d_pressed {
                    player.dir = PlayerDir::Right;
                }
                player.last_move_start = Some(self.ticks);
            } else {
                player.last_move_start = None;
            }
        }

        const MOVE_AMOUNT: f64 = 100.0;
        if self.a_pressed {
            player.move_horiz(-MOVE_AMOUNT * args.dt);
        } else if self.d_pressed {
            player.move_horiz(MOVE_AMOUNT * args.dt);
        }
        if self.w_pressed {
            player.move_vert(-MOVE_AMOUNT * args.dt)
        } else if self.s_pressed {
            player.move_vert(MOVE_AMOUNT * args.dt)
        }
    }
}

struct ScriptedCharacter {
    name: String,
    pos: (u32, u32),
    dir: u8,
}

enum Command {
    Pause(u32),
    Emote(String, u8),
    Move(String, (i32, i32), u8),
    Speak(String, String),
    GlobalFade,
    Viewport(i32, i32),
    Warp(String, (i32, i32)),
    FaceDirection(String, u8),
    ShowFrame(String, u32),
    Speed(String, u8),
    PlaySound(String),
    Shake(String, u32),
    Jump(String),
    TextAboveHead(String, String),
    AddQuest(u32),
    Message(String),
    Animate(String, bool, bool, u32, Vec<u32>),
    StopAnimation(String),
    Mail(String),
    Friendship(String, i32),
    PlayMusic(String),
    SpecificTemporarySprite(String),
    ChangeLocation(String),
    ChangeToTemporaryMap(String),
    Question(String, String),
    Fork(String),
    AmbientLight(u32, u32, u32),
    PositionOffset(String, i32, i32),
}

enum Trigger {
}

enum End {
    WarpOut,
    Dialogue(String, String),
    Position((u32, u32)),
    End,
}

struct ScriptedEvent {
    id: String,
    music: String,
    viewport: (i32, i32),
    characters: Vec<(String, (u32, u32), u8)>,
    skippable: bool,
    commands: Vec<Command>,
    end: End,
    triggers: Vec<Trigger>,
    forks: Vec<ScriptedEvent>,
}

fn parse_script(id: String, s: String) -> ScriptedEvent {
    let mut forks = s.split('\n');
    let mut parts = forks.next().unwrap().split('/');
    let music = parts.next().unwrap().to_owned();
    let viewport_str = parts.next().unwrap();
    let mut viewport_str = viewport_str.split(' ');
    let viewport = (viewport_str.next().unwrap().parse().unwrap(),
                    viewport_str.next().unwrap().parse().unwrap());
    let mut character_parts = parts.next().unwrap().split(' ').peekable();

    let mut characters = vec![];
    while character_parts.peek().is_some() {
        characters.push((character_parts.next().unwrap().to_owned(),
                         (character_parts.next().unwrap().parse().unwrap(),
                          character_parts.next().unwrap().parse().unwrap()),
                         character_parts.next().unwrap().parse().unwrap()));
    }

    let mut peekable = parts.peekable();
    let skippable = match peekable.peek() {
        Some(&"skippable") => {
            let _ = peekable.next();
            true
        }
        Some(_) | None => false,
    };

    let mut commands = vec![];
    for command_str in peekable {
        let args: Vec<_> = command_str.split(' ').collect();
        let command = match args[0] {
            "pause" => Command::Pause(args[1].parse().unwrap()),
            "emote" => Command::Emote(args[1].to_owned(), args[2].parse().unwrap()),
            "move" => Command::Move(args[1].to_owned(),
                                    (args[2].parse().unwrap(), args[3].parse().unwrap()),
                                    args[4].parse().unwrap()),
            "speak" => Command::Speak(args[1].to_owned(), args[2].to_owned()),
            "globalFade" => Command::GlobalFade,
            "viewport" => Command::Viewport(args[1].parse().unwrap(), args[2].parse().unwrap()),
            "warp" => Command::Warp(args[1].to_owned(),
                                    (args[2].parse().unwrap(), args[3].parse().unwrap())),
            "faceDirection" => Command::FaceDirection(args[1].to_owned(), args[2].parse().unwrap()),
            "showFrame" => Command::ShowFrame(args[1].to_owned(), args[2].parse().unwrap()),
            "speed" => Command::Speed(args[1].to_owned(), args[2].parse().unwrap()),
            "playSound" => Command::PlaySound(args[1].to_owned()),
            "shake" => Command::Shake(args[1].to_owned(), args[2].parse().unwrap()),
            "jump" => Command::Jump(args[1].to_owned()),
            "textAboveHead" => Command::TextAboveHead(args[1].to_owned(), args[2].to_owned()),
            "addQuest" => Command::AddQuest(args[1].parse().unwrap()),
            "message" => Command::Message(args[1].to_owned()),
            "animate" => Command::Animate(args[1].to_owned(),
                                          args[2] == "t",
                                          args[3] == "t",
                                          args[4].parse().unwrap(),
                                          args[5..].iter().map(|s| s.parse().unwrap()).collect()),
            "stopAnimation" => Command::StopAnimation(args[1].to_owned()),
            "mail" => Command::Mail(args[1].to_owned()),
            "friendship" => Command::Friendship(args[1].to_owned(), args[2].parse().unwrap()),
            "playMusic" => Command::PlayMusic(args[1].to_owned()),
            "specificTemporarySprite" => Command::SpecificTemporarySprite(args[1].to_owned()),
            "changeLocation" => Command::ChangeLocation(args[1].to_owned()),
            "changeToTemporaryMap" => Command::ChangeToTemporaryMap(args[1].to_owned()),
            "question" => Command::Question(args[1].to_owned(), args[2].to_owned()),
            "fork" => Command::Fork(args[1].to_owned()),
            "ambientLight" => Command::AmbientLight(args[1].parse().unwrap(),
                                                    args[2].parse().unwrap(),
                                                    args[3].parse().unwrap()),
            "positionOffset" => Command::PositionOffset(args[1].to_owned(),
                                                        args[2].parse().unwrap(),
                                                        args[3].parse().unwrap()),
            "end" => continue,
            s => panic!("unknown command {}", s),
        };
        commands.push(command);
    }

    ScriptedEvent {
        id: id,
        music: music,
        viewport: viewport,
        characters: characters,
        skippable: skippable,
        commands: commands,
        end: End::End, //XXXjdm
        triggers: vec![],
        forks: vec![],
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
    let map_name = args.next().unwrap_or("Town.xnb".into());
    let event_id = args.next();

    let base = Path::new("../xnb/uncompressed");
    let f = File::open(base.join("Maps").join(&map_name)).unwrap();
    let xnb = XNB::from_buffer(f).unwrap();
    let map = match xnb.primary {
        Asset::Tide(map) => map,
        _ => panic!("unexpected xnb contents"),
    };

    let event = event_id.and_then(|id| {
        let f = File::open(base.join("Data/Events").join(&map_name)).ok();
        let event = f.and_then(|f| {
            let xnb = XNB::from_buffer(f).unwrap();
            match xnb.primary {
                Asset::Dictionary(ref d) => {
                    for (k, v) in &d.map {
                        if let DictionaryKey::String(ref s) = *k {
                            if s.split('/').next() == Some(&id) {
                                if let Asset::String(ref s) = *v {
                                    return Some(s.clone());
                                }
                            }
                        }
                    }
                    None
                }
                _ => panic!("unexpected event xnb contents"),
            }
        });
        if let Some(ref event) = event {
            println!("got event source:\n{}", event);
        }
        event.map(|e| parse_script(id, e))
    });

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
    let base_dir_info = [Some(0), Some(2), Some(4), Some(2)];
    let mut player = Player {
        base: (base, 0, (16, 16), (0, 0), base_dir_info),
        bottom: (bottom, 24, (16, 16), (0, 16), base_dir_info),
        arms: (arms, 30, (16, 16), (0, 16), base_dir_info),
        hairstyle: (hairstyle, 0, (16, 16), (0, 0), base_dir_info),
        hat: (hat, 2, (20, 20), (-2, -2), [Some(0), Some(1), Some(3), Some(2)]),
        pants: (pants, 42, (16, 16), (0, 16), base_dir_info),
        shirt: (shirt, 0, (8, 8), (4, 15), [Some(0), Some(1), Some(3), Some(2)]),
        accessory: (accessory, 0, (16, 16), (0, 3), [Some(0), Some(1), None, Some(1)]),
        x: 10,
        y: 15,
        offset_x: 0.,
        offset_y: 0.,
        last_move_start: None,
        dir: PlayerDir::Down,
    };

    // Create a new game and run it.
    let mut app = App {
        gl: GlGraphics::new(opengl),
        view_x: 0,
        view_y: 0,
        ticks: 0,
        a_pressed: false,
        d_pressed: false,
        w_pressed: false,
        s_pressed: false,
        update_last_move: false,
    };

    let mut events = window.events();
    while let Some(e) = events.next(&mut window) {
        if let Some(Button::Keyboard(k)) = e.press_args() {
            match k {
                Key::Left if app.view_x > 0 => app.view_x -= 1,
                Key::Right => app.view_x += 1,
                Key::Up if app.view_y > 0 => app.view_y -= 1,
                Key::Down => app.view_y += 1,
                k => app.key_pressed(k),
            }
        }

        if let Some(Button::Keyboard(k)) = e.release_args() {
            app.key_released(k);
        }

        if let Some(r) = e.render_args() {
            app.render(&r, &tilesheets, &map.tilesheets, &player, &map.layers);
        }

        if let Some(u) = e.update_args() {
            app.update(&u, &mut player);
        }
    }
}
