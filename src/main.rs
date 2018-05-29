extern crate graphics;
extern crate piston_window;
extern crate image;
extern crate opengl_graphics;
extern crate piston;
extern crate squish;
extern crate xnb;

use graphics::Image;
use image::RgbaImage;
use opengl_graphics::{GlGraphics, OpenGL, Texture, TextureSettings, Filter, ImageSize};
use piston_window::{PistonWindow, WindowSettings, OpenGL as PistonOpenGL};
use piston::input::*;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::path::Path;
use squish::{decompress_image, CompressType};
use xnb::{XNB, SurfaceFormat, Texture2d, Dictionary};
use xnb::tide::{TileSheet, Layer, Map};

const SCALE: f64 = 1.5;

struct ResolvedTile<'a> {
    texture: &'a Texture,
    tilesheet: &'a TileSheet,
}

struct Character {
    _name: String,
    texture: TextureTileInfo,
    _index: u32,
    x: i32,
    y: i32,
    offset_x: f64,
    offset_y: f64,
    dir: PlayerDir,
}

pub struct App {
    gl: GlGraphics,
    view_x: i32,
    view_y: i32,
    view_w: u32,
    view_h: u32,
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

fn image_for_tile(tile: &Tile, pos: (i32, i32), view: (i32, i32)) -> Image {
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
                     pos: (i32, i32),
                     view: (i32, i32),
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
                            (x, y): (i32, i32),
                            (off_x, off_y): (i32, i32),
                            (view_x, view_y): (i32, i32),
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
        .src_rect([src_rect[0] as f64, src_rect[1] as f64, src_rect[2] as f64, src_rect[3] as f64])
        .rect([(x as i32 * 16) as f64 + off_x as f64 - view_x as f64,
               (y as i32 * 16) as f64 + off_y as f64 - view_y as f64,
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
    hat: Option<TextureTileInfo>,
    shirt: TextureTileInfo,
    accessory: TextureTileInfo,
    x: i32,
    y: i32,
    offset_x: f64,
    offset_y: f64,
    last_move_start: Option<u32>,
    dir: PlayerDir,
}

impl Player {
    fn adjusted_pos(&self, delta_x: f64, delta_y: f64) -> (i32, i32) {
        let x = self.x + (if delta_x < 0. && self.offset_x + delta_x < -8. {
            -1
        } else if delta_x > 0. && self.offset_x + delta_x > 8. {
           1
        } else {
            0
        });

        let y = self.y + (if delta_y < 0. && self.offset_y + delta_y < -8. {
            -1
        } else if delta_y > 0. && self.offset_y + delta_y > 8. {
           1
        } else {
            0
        });

        (x, y)
    }

    fn move_horiz(&mut self, delta: f64, clamp_to_current_pos: bool) {
        self.offset_x += delta;
        if delta < 0. && self.offset_x < -8. {
            if clamp_to_current_pos {
                self.offset_x = -7.99;
            } else {
                self.offset_x = 8.;
                self.x -= 1;
            }
        } else if delta > 0. && self.offset_x > 8. {
            if clamp_to_current_pos {
                self.offset_x = 7.99;
            } else {
                self.offset_x = -8.;
                self.x += 1;
            }
        }
    }

    fn move_vert(&mut self, delta: f64, clamp_to_current_pos: bool) {
        self.offset_y += delta;
        if delta < 0. && self.offset_y < -8. {
            if clamp_to_current_pos {
                self.offset_y = -7.99;
            } else {
                self.offset_y = 8.;
                self.y -= 1;
            }
        } else if delta > 0. && self.offset_y > 8. {
            if clamp_to_current_pos {
                self.offset_y = 7.99;
            } else {
                self.offset_y = -8.;
                self.y += 1;
            }
        }
    }
}


impl App {
    fn render(&mut self,
              args: &RenderArgs,
              player: &Player,
              characters: &[Character],
              layers: &[Layer],
              resolved_layers: &[Vec<ResolvedTile>]) {
        use graphics::*;

        const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

        let view_x = self.view_x;
        let view_y = self.view_y;

        self.view_w = args.viewport().window_size[0];
        self.view_h = args.viewport().window_size[1];
        let view_w = args.viewport().window_size[0] as i32 / 16 + view_x / 16;
        let view_h = args.viewport().window_size[1] as i32 / 16 + view_y / 16;

        let ticks = self.ticks;

        fn draw_character(character: &Character,
                          transform: [[f64; 3]; 2],
                          gl: &mut GlGraphics,
                          (view_x, view_y): (i32, i32),
                          (_view_w, _view_h): (i32, i32)) {
            if character.x < 0 || character.y < 0 {
                return;
            }
            let image = image_for_texture(&character.texture,
                                          (character.x, character.y),
                                          (view_x, view_y),
                                          (character.offset_x as i32, character.offset_y as i32),
                                          None,
                                          character.dir);
            image.draw(&character.texture.0, &Default::default(), transform, gl);
        }

        fn draw_layer(layer: &Layer,
                      resolved_tiles: &[ResolvedTile],
                      transform: [[f64; 3]; 2],
                      gl: &mut GlGraphics,
                      ticks: u32,
                      (view_x, view_y): (i32, i32),
                      (view_w, view_h): (i32, i32),
                      player: Option<&Player>) {
            if !layer.visible  || layer.id == "Paths" {
                return;
            }
            let mut last_pos = None;
            for (base_tile, resolved) in layer.tiles.iter().zip(resolved_tiles) {
                let tile = Tile {
                    sheet: resolved.tilesheet,
                    index: base_tile.get_index(ticks),
                };
                let (x, y) = base_tile.get_pos();
                let (x, y) = (x as i32, y as i32);

                if let Some(player) = player {
                    if y == player.y + 1 &&
                        x >= player.x &&
                        last_pos.map_or(false, |(tx, _)| tx < player.x)
                    {
                        draw_player(player, gl, transform.clone(), (view_x, view_y), ticks);
                    }
                }
                last_pos = Some((x, y));

                if x < view_x / 16 || x > view_w || y < view_y / 16 || y > view_h {
                    continue;
                }
                let image = image_for_tile(&tile, (x, y), (view_x, view_y));
                image.draw(resolved.texture, &Default::default(), transform, gl);
            }
        }

        fn draw_player(
            player: &Player,
            gl: &mut GlGraphics,
            transform: [[f64; 3]; 2],
            view: (i32, i32),
            ticks: u32,
        ) {
            let pos = (player.x as i32, player.y as i32);
            let offset = (player.offset_x as i32, player.offset_y as i32);

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
            if let Some(ref hat) = player.hat {
                let image = image_for_texture(hat, pos, view, offset, None, player.dir);
                image.draw(&hat.0, &Default::default(), transform, gl);
            }

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
        }

        self.gl.draw(args.viewport(), |c, gl| {
            // Clear the screen.
            clear(BLACK, gl);

            let transform = c.transform.zoom(SCALE);

            for (i, (layer, resolved)) in layers.iter().zip(resolved_layers).enumerate() {
                if i == layers.len() - 1 {
                    break;
                }
                let player = if i == 1 { Some(player) } else { None };
                draw_layer(layer, resolved, transform, gl, ticks,
                           (view_x, view_y), (view_w, view_h), player);
            }

            for character in characters {
                draw_character(character, transform, gl,
                               (view_x, view_y), (view_w, view_h));
            }

            draw_layer(layers.last().unwrap(),
                       resolved_layers.last().unwrap(),
                       transform, gl, ticks,
                       (view_x, view_y), (view_w, view_h), None);
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

    fn update(&mut self, args: &UpdateArgs, player: &mut Player, map: &Map) {
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
        let delta_x = if self.a_pressed {
            -MOVE_AMOUNT * args.dt
        } else if self.d_pressed {
            MOVE_AMOUNT * args.dt
        } else {
            0.
        };

        let delta_y = if self.w_pressed {
            -MOVE_AMOUNT * args.dt
        } else if self.s_pressed {
            MOVE_AMOUNT * args.dt
        } else {
            0.
        };

        let (adjusted_x, adjusted_y) = player.adjusted_pos(delta_x, delta_y);
        let layer = map.layers.iter().find(|l| l.id == "Buildings").expect("no buildings?");
        let mut clamp_to_current_pos = false;
        for tile in &layer.tiles {
            let (tx, ty) = tile.get_pos();
            if (tx as i32, ty as i32) == (adjusted_x, adjusted_y + 1) {
                clamp_to_current_pos = true;
                break;
            }
        }

        player.move_horiz(delta_x, clamp_to_current_pos);
        player.move_vert(delta_y, clamp_to_current_pos);

        let player_x = player.x * 16 + player.offset_x as i32;
        let player_y = player.y * 16 + player.offset_y as i32;

        let (view_w, view_h) = ((self.view_w as f64 / SCALE) as i32, (self.view_h as f64 / SCALE)  as i32);

        let adjusted_x = if player_x - self.view_x < view_w / 3 {
            player_x - view_w / 3
        } else if player_x - self.view_x > view_w / 3 * 2 {
            player_x - view_w / 3 * 2
        } else {
            self.view_x
        };

        let adjusted_y = if player_y - self.view_y < view_h / 3 {
            player_y - view_h / 3
        } else if player_y - self.view_y > view_h / 3 * 2 {
            player_y - view_h / 3 * 2
        } else {
            self.view_y
        };

        let max_x = (map.layers[0].size.0 as i32 - view_w / 16) * 16;
        let max_y = (map.layers[0].size.1 as i32 - view_h / 16) * 16;
        self.view_x = adjusted_x.max(0).min(max_x);
        self.view_y = adjusted_y.max(0).min(max_y);
    }
}

struct ScriptedCharacter {
    name: String,
    pos: (i32, i32),
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

#[allow(dead_code)]
enum End {
    WarpOut,
    Dialogue(String, String),
    Position((u32, u32)),
    End,
}

struct ScriptedEvent {
    _id: String,
    _music: String,
    viewport: (i32, i32),
    characters: Vec<ScriptedCharacter>,
    _skippable: bool,
    _commands: Vec<Command>,
    _end: End,
    _triggers: Vec<Trigger>,
    _forks: Vec<ScriptedEvent>,
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
        let character = ScriptedCharacter {
            name: character_parts.next().unwrap().to_owned(),
            pos: (character_parts.next().unwrap().parse().unwrap(),
                  character_parts.next().unwrap().parse().unwrap()),
            dir: character_parts.next().unwrap().parse().unwrap(),
        };
        characters.push(character);
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
            "" => continue,
            s => panic!("unknown command {}", s),
        };
        commands.push(command);
    }

    ScriptedEvent {
        _id: id,
        _music: music,
        viewport: viewport,
        characters: characters,
        _skippable: skippable,
        _commands: commands,
        _end: End::End, //XXXjdm
        _triggers: vec![],
        _forks: vec![],
    }
}

fn characters_for_event(event: &ScriptedEvent, path: &Path) -> Vec<Character> {
    let mut characters = vec![];
    for character in &event.characters {
        if character.name == "farmer" {
            continue;
        }
        let texture = load_texture(path, &format!("{}.xnb", character.name));
        let info = (texture, 0, (16, 32), (0, 0), [Some(0), Some(1), Some(2), Some(3)]);
        characters.push(Character {
            texture: info,
            _name: character.name.clone(),
            x: character.pos.0,
            y: character.pos.1,
            offset_x: 0.,
            offset_y: 0.,
            _index: 0,
            dir: match character.dir {
                0 => PlayerDir::Up,
                1 => PlayerDir::Right,
                2 => PlayerDir::Down,
                3 => PlayerDir::Left,
                _ => unreachable!(),
            },
        });
    }
    characters
}

fn load_texture(base: &Path, filename: &str) -> Texture {
    let mut f = File::open(base.join(filename)).unwrap();
    let xnb = XNB::<Texture2d>::from_buffer(&mut f).unwrap();
    let mut texture = xnb.primary;
    let data = texture.mip_data.remove(0);
    let data = match texture.format {
        SurfaceFormat::Dxt3 => {
            decompress_image(texture.width as i32,
                             texture.height as i32,
                             data.as_ptr() as *const _,
                             CompressType::Dxt3)
        }
        _ => data,
    };
    let img = RgbaImage::from_raw(texture.width as u32,
                                  texture.height as u32,
                                  data).unwrap();
    let mut settings = TextureSettings::new();
    settings.set_filter(Filter::Nearest);
    Texture::from_image(&img, &settings)
}

fn main() {
    // Create an Glutin window.
    const WINDOW_DIMENSIONS: (u32, u32) = (800, 600);
    let mut window: PistonWindow = WindowSettings::new(
            "spinning-square",
            [WINDOW_DIMENSIONS.0, WINDOW_DIMENSIONS.1]
        )
        .opengl(PistonOpenGL::V3_2)
        .exit_on_esc(true)
        .vsync(true)
        .build()
        .unwrap();

    let mut args = env::args();
    let _self = args.next();
    let map_name = args.next().unwrap_or("Town.xnb".into());
    let event_id = args.next();

    let mut view_x = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut view_y = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);

    let base = Path::new("../xnb/uncompressed");
    let mut f = File::open(base.join("Maps").join(&map_name)).unwrap();
    let xnb = XNB::<Map>::from_buffer(&mut f).unwrap();
    let mut map = xnb.primary;

    for layer in &mut map.layers {
        layer.tiles.sort_by(|t1, t2| {
            let (t1_x, t1_y) = t1.get_pos();
            let (t2_x, t2_y) = t2.get_pos();
            t1_y.cmp(&t2_y).then_with(|| t1_x.cmp(&t2_x))
        });
    }

    let event = event_id.and_then(|id| {
        let f = File::open(base.join("Data/Events").join(&map_name)).ok();
        let event = f.and_then(|mut f| {
            let xnb = XNB::<Dictionary<String, String>>::from_buffer(&mut f).unwrap();
            for (k, v) in &xnb.primary.map {
                if k.split('/').next() == Some(&id) {
                    return Some(v.clone());
                }
            }
            None
        });
        if let Some(ref event) = event {
            println!("got event source:\n{}", event);
        }
        event.map(|e| parse_script(id, e))
    });

    let mut tilesheets = HashMap::new();
    for ts in &map.tilesheets {
        let texture = load_texture(base, &format!("{}.xnb", ts.image_source));
        println!("storing texture for {}", ts.id);
        tilesheets.insert(ts.id.clone(), texture);
    }
    println!("loaded {} tilesheets", tilesheets.len());

    let mut resolved_layers = vec![];
    for layer in &map.layers {
        let layer_tiles = layer.tiles.iter().map(|t| {
            let name = t.get_tilesheet();
            ResolvedTile {
                texture: tilesheets.get(name).expect("missing texture"),
                tilesheet: map.tilesheets.iter().find(|s| s.id == name).expect("missing tilesheet"),
            }
        }).collect();
        resolved_layers.push(layer_tiles);
    }

    let character_path = Path::new("../xnb/uncompressed/Characters");
    let path = character_path.join("Farmer");
    let base = load_texture(&path, "farmer_base.xnb");
    let bottom = load_texture(&path, "farmer_base.xnb");
    let arms = load_texture(&path, "farmer_base.xnb");
    let pants = load_texture(&path, "farmer_base.xnb");
    let hairstyle = load_texture(&path, "hairstyles.xnb");
    //let hat = load_texture(&path, "hats.xnb");
    let shirt = load_texture(&path, "shirts.xnb");
    let accessory = load_texture(&path, "accessories.xnb");
    let base_dir_info = [Some(0), Some(2), Some(4), Some(2)];
    let mut player = Player {
        base: (base, 0, (16, 16), (0, 0), base_dir_info),
        bottom: (bottom, 24, (16, 16), (0, 16), base_dir_info),
        arms: (arms, 30, (16, 16), (0, 16), base_dir_info),
        hairstyle: (hairstyle, 0, (16, 16), (0, 0), base_dir_info),
        //hat: (hat, 2, (20, 20), (-2, -2), [Some(0), Some(1), Some(3), Some(2)]),
        hat: None,
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

    let characters = match event {
        Some(ref ev) => characters_for_event(ev, &character_path),
        None => vec![],
    };

    if let Some(ref event) = event {
        view_x = event.viewport.0;
        view_y = event.viewport.1;
    }

    // Create a new game and run it.
    let mut app = App {
        gl: GlGraphics::new(OpenGL::V3_2),
        view_x: view_x * map.tilesheets[0].tile_size.0 as i32,
        view_y: view_y * map.tilesheets[0].tile_size.1 as i32,
        view_w: WINDOW_DIMENSIONS.0,
        view_h: WINDOW_DIMENSIONS.1,
        ticks: 0,
        a_pressed: false,
        d_pressed: false,
        w_pressed: false,
        s_pressed: false,
        update_last_move: false,
    };

    while let Some(e) = window.next() {
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
            app.render(&r,
                       &player,
                       &characters,
                       &map.layers,
                       &resolved_layers);
        }

        if let Some(u) = e.update_args() {
            app.update(&u, &mut player, &map);
        }
    }
}
