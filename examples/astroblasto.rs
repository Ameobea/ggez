//! An Asteroids-ish example game to show off ggez.
//! The idea is that this game is simple but still
//! non-trivial enough to be interesting.

extern crate ggez;
extern crate rand;

use std::path;

// TODO: Can we re-export these types from game.rs
// instead of requring this use?
// use sdl2::keyboard::Keycode;

use ggez::audio;
use ggez::conf;
use ggez::event::*;
use ggez::game::{Game, GameState};
// use ggez::game::Keycode;
use ggez::{GameResult, Context};
use ggez::graphics;
use ggez::timer;
use std::time::Duration;
use std::ops::{Add, AddAssign, Sub};


#[derive(Debug, Copy, Clone)]
struct Vec2 {
    x: f64,
    y: f64,
}

impl Vec2 {
    fn new(x: f64, y: f64) -> Self {
        Vec2 { x: x, y: y }
    }

    /// Create a unit vector representing the
    /// given angle (in radians)
    fn from_angle(angle: f64) -> Self {
        let vx = angle.sin();
        let vy = angle.cos();
        Vec2 { x: vx, y: vy }
    }

    fn random(max_magnitude: f64) -> Self {
        let angle = rand::random::<f64>() * 2.0 * std::f64::consts::PI;
        let mag = rand::random::<f64>() * max_magnitude;
        Vec2::from_angle(angle).scaled(mag)
    }

    fn magnitude(&self) -> f64 {
        ((self.x * self.x) + (self.y * self.y)).sqrt()
    }

    fn normalized(&self) -> Self {
        let mag = self.magnitude();
        self.scaled(1.0 / mag)
    }

    fn scaled(&self, rhs: f64) -> Self {
        Vec2 {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }

    /// Returns a vector whose magnitude is between
    /// 0 and max.
    fn clamped(&self, max: f64) -> Self {
        let mag = self.magnitude();
        if mag > max {
            self.normalized().scaled(max)
        } else {
            *self
        }
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Vec2) -> Self {
        Vec2 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}


impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Vec2) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}


impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Vec2) -> Self {
        Vec2 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Default for Vec2 {
    fn default() -> Self {
        Self::new(0., 0.)
    }
}

#[derive(Debug)]
enum ActorType {
    Player,
    Rock,
    Shot,
}

#[derive(Debug)]
struct Actor {
    tag: ActorType,
    pos: Vec2,
    facing: f64,
    velocity: Vec2,
    rvel: f64,
    bbox_size: f64,

    // I am going to lazily overload "life" with a
    // double meaning rather than making a proper ECS;
    // for shots, it is the time left to live,
    // for players and such, it is the actual hit points.
    life: f64,
}

const PLAYER_LIFE: f64 = 1.0;
const SHOT_LIFE: f64 = 2.0;
const ROCK_LIFE: f64 = 1.0;

const PLAYER_BBOX: f64 = 12.0;
const ROCK_BBOX: f64 = 12.0;
const SHOT_BBOX: f64 = 6.0;

fn create_player() -> Actor {
    Actor {
        tag: ActorType::Player,
        pos: Vec2::default(),
        facing: 0.,
        velocity: Vec2::default(),
        rvel: 0.,
        bbox_size: PLAYER_BBOX,
        life: PLAYER_LIFE,
    }
}

fn create_rock() -> Actor {
    Actor {
        tag: ActorType::Rock,
        pos: Vec2::default(),
        facing: 0.,
        velocity: Vec2::default(),
        rvel: 0.,
        bbox_size: ROCK_BBOX,
        life: ROCK_LIFE,
    }
}

fn create_shot() -> Actor {
    Actor {
        tag: ActorType::Shot,
        pos: Vec2::default(),
        facing: 0.,
        velocity: Vec2::default(),
        rvel: SHOT_RVEL,
        bbox_size: SHOT_BBOX,
        life: SHOT_LIFE,
    }
}

const MAX_ROCK_VEL: f64 = 50.0;

/// Create the given number of rocks.
/// Makes sure that none of them are within the
/// given exclusion zone (nominally the player)
/// Note that this *could* create rocks outside the
/// bounds of the playing field, so it should be 
/// called before `wrap_actor_position()` happens.
fn create_rocks(num: i32, exclusion: &Vec2, min_radius: f64, max_radius: f64) -> Vec<Actor> {
    assert!(max_radius > min_radius);
    let new_rock = |_| {
        let mut rock = create_rock();
        let r_angle = rand::random::<f64>() * 2.0 * std::f64::consts::PI;
        let r_distance = rand::random::<f64>() * (max_radius - min_radius) + min_radius;
        rock.pos = Vec2::from_angle(r_angle).scaled(r_distance) + *exclusion;
        rock.velocity = Vec2::random(MAX_ROCK_VEL);
        rock
    };
    (0..num).map(new_rock).collect()
}

const SHOT_SPEED: f64 = 200.0;
const SHOT_RVEL: f64 = 0.1;
const SPRITE_SIZE: u32 = 32;

// Acceleration in pixels per second, more or less.
const PLAYER_THRUST: f64 = 100.0;
// Rotation in radians per second.
const PLAYER_TURN_RATE: f64 = 3.05;
// Seconds between shots
const PLAYER_SHOT_TIME: f64 = 0.5;


fn player_handle_input(actor: &mut Actor, input: &InputState, dt: f64) {
    actor.facing += dt * PLAYER_TURN_RATE * input.xaxis;

    if input.yaxis > 0.0 {
        player_thrust(actor, dt);
    }
}

fn player_thrust(actor: &mut Actor, dt: f64) {
    let direction_vector = Vec2::from_angle(actor.facing);
    let thrust_vector = direction_vector.scaled(PLAYER_THRUST);
    actor.velocity += thrust_vector.scaled(dt);
}

const MAX_PHYSICS_VEL: f64 = 250.0;

fn update_actor_position(actor: &mut Actor, dt: f64) {
    actor.velocity = actor.velocity.clamped(MAX_PHYSICS_VEL);
    let dv = actor.velocity.scaled(dt);
    actor.pos += dv;
    actor.facing += actor.rvel;
}

fn wrap_actor_position(actor: &mut Actor, sx: f64, sy: f64) {
    // Wrap screen
    let screen_x_bounds = sx / 2.0;
    let screen_y_bounds = sy / 2.0;
    let sprite_half_size = (SPRITE_SIZE / 2) as f64;
    let actor_center = actor.pos - Vec2::new(-sprite_half_size, sprite_half_size);
    if actor_center.x > screen_x_bounds {
        actor.pos.x -= sx;
    } else if actor_center.x < -screen_x_bounds {
        actor.pos.x += sx;
    };
    if actor_center.y > screen_y_bounds {
        actor.pos.y -= sy;
    } else if actor_center.y < -screen_y_bounds {
        actor.pos.y += sy;
    }
}

fn handle_timed_life(actor: &mut Actor, dt: f64) {
    actor.life -= dt;
}


// Translates the world coordinate system, which
// has Y pointing up and the origin at the center,
// to the screen coordinate system, which has Y
// pointing downward and the origin at the top-left,

fn world_to_screen_coords(state: &MainState, point: &Vec2) -> Vec2 {
    let width = state.screen_width as f64;
    let height = state.screen_height as f64;
    let x = point.x + width / 2.0;
    let y = height - (point.y + height / 2.0);
    Vec2 { x: x, y: y }
}


struct Assets {
    player_image: graphics::Image,
    shot_image: graphics::Image,
    rock_image: graphics::Image,
    font: graphics::Font,
}

impl Assets {
    fn new(ctx: &mut Context) -> GameResult<Assets> {
        let player_image_path = path::Path::new("player.png");
        let player_image = try!(graphics::Image::new(ctx, player_image_path));
        let shot_image_path = path::Path::new("shot.png");
        let shot_image = try!(graphics::Image::new(ctx, shot_image_path));
        let rock_image_path = path::Path::new("rock.png");
        let rock_image = try!(graphics::Image::new(ctx, rock_image_path));
        let font_path = path::Path::new("consolefont.png");
        let font_chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!,.?;'\"";
        let font = try!(graphics::Font::new_bitmap(ctx, font_path, font_chars));
        Ok(Assets {
            player_image: player_image,
            shot_image: shot_image,
            rock_image: rock_image,
            font: font,
        })
    }
}

#[derive(Debug)]
struct InputState {
    xaxis: f64,
    yaxis: f64,
    fire: bool,
}

impl Default for InputState {
    fn default() -> Self {
        InputState {
            xaxis: 0.0,
            yaxis: 0.0,
            fire: false,
        }
    }
}

struct MainState {
    player: Actor,
    shots: Vec<Actor>,
    rocks: Vec<Actor>,
    level: i32,
    score: i32,
    assets: Assets,
    screen_width: u32,
    screen_height: u32,
    input: InputState,
    player_shot_timeout: f64, 
    gui_dirty: bool,
    score_display: graphics::Text,
    level_display: graphics::Text,
}



impl MainState {
    fn fire_player_shot(&mut self) {
        self.player_shot_timeout = PLAYER_SHOT_TIME;

        let player = &self.player;
        let mut shot = create_shot();
        shot.pos = player.pos;
        shot.facing = player.facing;
        let direction = Vec2::from_angle(shot.facing);
        shot.velocity.x = SHOT_SPEED * direction.x;
        shot.velocity.y = SHOT_SPEED * direction.y;

        self.shots.push(shot);
    }


    fn draw_actor(&self, ctx: &mut Context, actor: &Actor) -> GameResult<()> {

        let pos = world_to_screen_coords(self, &actor.pos);
        let px = pos.x as i32;
        let py = pos.y as i32;
        let destrect = graphics::Rect::new(px, py, SPRITE_SIZE, SPRITE_SIZE);
        let actor_center = graphics::Point::new(16, 16);
        let image = self.actor_image(actor);
        graphics::draw_ex(ctx,
                          image,
                          None,
                          Some(destrect),
                          actor.facing.to_degrees(),
                          Some(actor_center),
                          false,
                          false)

    }

    fn actor_image(&self, actor: &Actor) -> &graphics::Image {
        match actor.tag {
            ActorType::Player => &self.assets.player_image,
            ActorType::Rock => &self.assets.rock_image,
            ActorType::Shot => &self.assets.shot_image,
        }
    }

    fn clear_dead_stuff(&mut self) {
        self.shots.retain(|s| s.life > 0.0);
        self.rocks.retain(|r| r.life > 0.0);
    }

    fn handle_collisions(&mut self) {
        for rock in &mut self.rocks {
            let pdistance = rock.pos - self.player.pos;
            if pdistance.magnitude() < (self.player.bbox_size + rock.bbox_size) {
                self.player.life = 0.0;
            }
            for shot in &mut self.shots {
                let distance = shot.pos - rock.pos;
                if distance.magnitude() < (shot.bbox_size + rock.bbox_size) {
                    shot.life = 0.0;
                    rock.life = 0.0;
                    self.score += 1;
                    self.gui_dirty = true;
                }
            }
        }
    }

    fn check_for_level_respawn(&mut self) {
        if self.rocks.len() == 0 {
            self.level += 1;
            self.gui_dirty = true;
            let r = create_rocks(self.level + 5, &self.player.pos, 100.0, 250.0);
            self.rocks.extend(r);
        }
    }

    fn update_ui(&mut self, ctx: &Context) {
        let score_str = format!("SCORE{}", self.score);
        let level_str = format!("LEVEL{}", self.level);
        let score_text = graphics::Text::new(ctx, &score_str, &self.assets.font).unwrap();
        let level_text = graphics::Text::new(ctx, &level_str, &self.assets.font).unwrap();

        self.score_display = score_text;
        self.level_display = level_text;
    }
}

impl<'a> GameState for MainState {
    fn load(ctx: &mut Context, conf: &conf::Conf) -> GameResult<MainState> {
        ctx.print_sound_stats();
        ctx.print_resource_stats();
        graphics::set_background_color(ctx, graphics::Color::RGB(0, 0, 0));

        println!("Game resource path: {:?}", ctx.filesystem);

        let assets = try!(Assets::new(ctx));
        let score_disp = try!(graphics::Text::new(ctx, "score", &assets.font));
        let level_disp = try!(graphics::Text::new(ctx, "level", &assets.font));

        let player = create_player();
        let rocks = create_rocks(5, &player.pos, 100.0, 250.0);

        let s = MainState {
            player: player,
            shots: Vec::new(),
            rocks: rocks,
            level: 0,
            score: 0,
            assets: assets,
            screen_width: conf.window_width,
            screen_height: conf.window_height,
            input: InputState::default(),
            player_shot_timeout: 0.0, 
            gui_dirty: true,
            score_display: score_disp,
            level_display: level_disp, 
        };

        Ok(s)
    }

    fn update(&mut self, ctx: &mut Context, dt: Duration) -> GameResult<()> {
        // println!("Player: {:?}", self.player);
        let seconds = timer::duration_to_f64(dt);
        player_handle_input(&mut self.player, &mut self.input, seconds);
        self.player_shot_timeout -= seconds;
        if self.input.fire && self.player_shot_timeout < 0.0 {
            self.fire_player_shot();
        }

        update_actor_position(&mut self.player, seconds);
        wrap_actor_position(&mut self.player,
                            self.screen_width as f64,
                            self.screen_height as f64);

        // let mut dead_shots = Vec::new();
        for act in &mut self.shots {
            update_actor_position(act, seconds);
            wrap_actor_position(act, self.screen_width as f64, self.screen_height as f64);
            handle_timed_life(act, seconds);
        }

        for act in &mut self.rocks {
            update_actor_position(act, seconds);
            wrap_actor_position(act, self.screen_width as f64, self.screen_height as f64);
        }

        self.handle_collisions();

        self.clear_dead_stuff();

        self.check_for_level_respawn();

        if self.gui_dirty {
            self.update_ui(ctx);
            self.gui_dirty = false;
        }

        if self.player.life == 0.0 {
            println!("Game over!");
            // ctx.quit() is broken.  ;_;
            ctx.quit();
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx);

        let p = &self.player;
        try!(self.draw_actor(ctx, p));
        for s in &self.shots {
            try!(self.draw_actor(ctx, &s));
        }
        for r in &self.rocks {
            try!(self.draw_actor(ctx, &r));
        }


        let level_rect = graphics::Rect::new(0, 0, self.level_display.width(), self.level_display.height());
        let score_rect = graphics::Rect::new(200, 0, self.score_display.width(), self.score_display.height());
        graphics::draw(ctx, &self.level_display, None, Some(level_rect));
        graphics::draw(ctx, &self.score_display, None, Some(score_rect));

        graphics::present(ctx);
        timer::sleep_until_next_frame(ctx, 60);
        Ok(())
    }

    fn key_down_event(&mut self, keycode: Option<Keycode>, _keymod: Mod, _repeat: bool) {
        match keycode {
            Some(Keycode::Up) => {
                self.input.yaxis = 1.0;
            }
            Some(Keycode::Left) => {
                self.input.xaxis = -1.0;
            }
            Some(Keycode::Right) => {
                self.input.xaxis = 1.0;
            }
            Some(Keycode::Space) => {
                self.input.fire = true;
            }
            _ => (), // Do nothing
        }
    }


    fn key_up_event(&mut self, keycode: Option<Keycode>, _keymod: Mod, _repeat: bool) {
        match keycode {
            Some(Keycode::Up) => {
                self.input.yaxis = 0.0;
            }
            Some(Keycode::Left) => {
                self.input.xaxis = 0.0;
            }
            Some(Keycode::Right) => {
                self.input.xaxis = 0.0;
            }
            Some(Keycode::Space) => {
                self.input.fire = false;
            }
            _ => (), // Do nothing
        }
    }
}

pub fn main() {
    let mut c = conf::Conf::new();
    c.window_title = "Astroblasto!".to_string();
    c.window_width = 640;
    c.window_height = 480;
    c.window_icon = "player.png".to_string();
    let game: GameResult<Game<MainState>> = Game::new("astroblasto", c);
    match game {
        Err(e) => {
            println!("Could not load game!");
            println!("Error: {:?}", e);
        }
        Ok(mut game) => {
            let result = game.run();
            if let Err(e) = result {
                println!("Error encountered running game: {:?}", e);
            } else {
                println!("Game exited cleanly.");
            }
        }
    }
}