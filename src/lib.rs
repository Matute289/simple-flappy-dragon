use bracket_lib::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const FRAME_DURATION: f32 = 75.0;

// --- WASM-only: JS import and restart flag ---

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    fn on_game_over(score: u32);
}

#[cfg(target_arch = "wasm32")]
use std::cell::Cell;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static INITIALIZED: Cell<bool> = Cell::new(false);
    static RESTART_REQUESTED: Cell<bool> = Cell::new(false);
}

/// Called by JS when the user clicks PLAY.
/// First call initializes bracket-lib; subsequent calls restart the game.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start_game() {
    console_error_panic_hook::set_once();

    let already_running = INITIALIZED.with(|i| i.get());
    if already_running {
        RESTART_REQUESTED.with(|r| r.set(true));
        return;
    }
    INITIALIZED.with(|i| i.set(true));
    run().expect("Game initialization failed");
}

// --- Core game ---

pub fn run() -> BError {
    let context = BTermBuilder::simple80x50()
        .with_title("Flappy Dragon")
        .build()?;
    main_loop(context, State::new())
}

struct State {
    player: Player,
    frame_time: f32,
    obstacle: Obstacle,
    mode: GameMode,
    score: i32,
    #[cfg(target_arch = "wasm32")]
    game_over_reported: bool,
}

impl State {
    fn new() -> Self {
        State {
            player: Player::new(5, 25),
            frame_time: 0.0,
            obstacle: Obstacle::new(SCREEN_WIDTH, 0),
            mode: GameMode::Playing,
            score: 0,
            #[cfg(target_arch = "wasm32")]
            game_over_reported: false,
        }
    }

    fn play(&mut self, ctx: &mut BTerm) {
        ctx.cls_bg(NAVY);
        self.frame_time += ctx.frame_time_ms;
        if self.frame_time > FRAME_DURATION {
            self.frame_time = 0.0;
            self.player.gravity_and_move();
        }
        if let Some(VirtualKeyCode::Space) = ctx.key {
            self.player.flap();
        }
        self.player.render(ctx);
        ctx.print(0, 0, "Press SPACE to flap.");
        ctx.print(0, 1, &format!("Score: {}", self.score));
        self.obstacle.render(ctx, self.player.x);
        if self.player.x > self.obstacle.x {
            self.score += 1;
            self.obstacle = Obstacle::new(self.player.x + SCREEN_WIDTH, self.score);
        }
        if self.player.y > SCREEN_HEIGHT || self.obstacle.hit_obstacle(&self.player) {
            self.mode = GameMode::End;
        }
    }

    fn dead(&mut self, ctx: &mut BTerm) {
        #[cfg(target_arch = "wasm32")]
        if !self.game_over_reported {
            on_game_over(self.score as u32);
            self.game_over_reported = true;
        }
        // Clear canvas — HTML overlay appears on top
        ctx.cls();
    }
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        // Check for restart request from JS
        #[cfg(target_arch = "wasm32")]
        {
            let restart = RESTART_REQUESTED.with(|r| {
                if r.get() {
                    r.set(false);
                    true
                } else {
                    false
                }
            });
            if restart {
                *self = State::new();
                return;
            }
        }

        match self.mode {
            GameMode::Playing => self.play(ctx),
            GameMode::End => self.dead(ctx),
        }
    }
}

enum GameMode {
    Playing,
    End,
}

struct Player {
    x: i32,
    y: i32,
    velocity: f32,
}

impl Player {
    fn new(x: i32, y: i32) -> Self {
        Player { x, y, velocity: 0.0 }
    }

    fn render(&mut self, ctx: &mut BTerm) {
        ctx.set(0, self.y, YELLOW, BLACK, to_cp437('@'));
    }

    fn gravity_and_move(&mut self) {
        if self.velocity < 2.0 {
            self.velocity += 0.5;
        }
        self.y += self.velocity as i32;
        self.x += 2;
        if self.y < 0 {
            self.y = 0;
        }
    }

    fn flap(&mut self) {
        self.velocity = -3.0;
    }
}

struct Obstacle {
    x: i32,
    gap_y: i32,
    size: i32,
}

impl Obstacle {
    fn new(x: i32, score: i32) -> Self {
        let mut random = RandomNumberGenerator::new();
        Obstacle {
            x,
            gap_y: random.range(10, 40),
            size: i32::max(2, 20 - score),
        }
    }

    fn render(&mut self, ctx: &mut BTerm, player_x: i32) {
        let screen_x = self.x - player_x;
        let half_size = self.size / 2;
        for y in 0..self.gap_y - half_size {
            ctx.set(screen_x, y, RED, BLACK, to_cp437('|'));
        }
        for y in self.gap_y + half_size..SCREEN_HEIGHT {
            ctx.set(screen_x, y, RED, BLACK, to_cp437('|'));
        }
    }

    fn hit_obstacle(&self, player: &Player) -> bool {
        let half_size = self.size / 2;
        let does_x_match = player.x == self.x;
        let player_above_gap = player.y < self.gap_y - half_size;
        let player_below_gap = player.y > self.gap_y + half_size;
        does_x_match && (player_above_gap || player_below_gap)
    }
}
