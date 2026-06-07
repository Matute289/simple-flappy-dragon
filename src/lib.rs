use bracket_lib::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
#[allow(dead_code)]
const FRAME_DURATION: f32 = 75.0;

// --- Pure logic helpers (no bracket-lib context; tested via `cargo test`) ---

#[allow(dead_code)]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[allow(dead_code)]
const SKY_COLORS: [(u8, u8, u8); 6] = [
    (255, 100,  50),  // 0 Dawn
    (255, 210, 120),  // 1 Morning
    ( 80, 160, 255),  // 2 Noon
    (140, 200, 255),  // 3 Afternoon
    (220,  90,  40),  // 4 Dusk
    ( 10,  10,  55),  // 5 Night
];
#[allow(dead_code)]
const SKY_PHASE_MS: f32 = 30_000.0;
#[allow(dead_code)]
const SKY_TOTAL_MS: f32 = SKY_PHASE_MS * 6.0; // 180_000 ms = 3 minutes
#[allow(dead_code)]
const CLOUD_ACTIVATION_MS: f32 = SKY_TOTAL_MS; // clouds start after 1 full cycle

#[allow(dead_code)]
fn sky_bg_color_at(sky_time_ms: f32) -> RGB {
    let t = sky_time_ms % SKY_TOTAL_MS;
    let phase_f = t / SKY_PHASE_MS;
    let phase = phase_f as usize % 6;
    let phase_t = phase_f - phase as f32;
    let next = (phase + 1) % 6;
    let (r1, g1, b1) = SKY_COLORS[phase];
    let (r2, g2, b2) = SKY_COLORS[next];
    RGB::from_f32(
        lerp(r1 as f32 / 255.0, r2 as f32 / 255.0, phase_t),
        lerp(g1 as f32 / 255.0, g2 as f32 / 255.0, phase_t),
        lerp(b1 as f32 / 255.0, b2 as f32 / 255.0, phase_t),
    )
}

#[allow(dead_code)]
fn frame_duration_for(score: i32, classic: bool) -> f32 {
    if classic { return 75.0; }
    (75.0 - score as f32 * 1.5).max(30.0)
}

#[allow(dead_code)]
fn gap_size_for(score: i32, classic: bool) -> i32 {
    if classic {
        i32::max(2, 20 - score)
    } else {
        i32::max(4, 16 - (score / 20) * 2)
    }
}

#[allow(dead_code)]
fn player_x_speed_for(score: i32, classic: bool) -> i32 {
    if classic { return 2; }
    2 + (score / 10)
}

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
    static PLAYER_Y: Cell<i32> = Cell::new(25);
}

/// Returns the player's current row (0–49) so JS can position the dragon overlay.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_player_y() -> i32 {
    PLAYER_Y.with(|y| y.get())
}

/// Called by JS when the user clicks PLAY.
/// First call initializes bracket-lib; subsequent calls restart the game.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start_game(classic: bool) {
    console_error_panic_hook::set_once();
    let already_running = INITIALIZED.with(|i| i.get());
    if already_running {
        RESTART_REQUESTED.with(|r| r.set(true));
        return;
    }
    INITIALIZED.with(|i| i.set(true));
    run(classic).expect("Game initialization failed");
}

// --- Core game ---

pub fn run(classic: bool) -> BError {
    let context = BTermBuilder::simple80x50()
        .with_title("Flappy Dragon")
        .build()?;
    main_loop(context, State::new(classic))
}

struct Cloud {
    x: f32,    // screen-space x (0..80), can be negative while drifting off-screen
    y: i32,    // row in the terminal grid (3–9)
    width: i32,
}

struct State {
    player: Player,
    frame_time: f32,
    obstacle: Obstacle,
    mode: GameMode,
    score: i32,
    classic_mode: bool,
    sky_time: f32,
    weather_state: u8,
    weather_timer: f32,
    clouds: Vec<Cloud>,
    rng: RandomNumberGenerator,
    #[cfg(target_arch = "wasm32")]
    game_over_reported: bool,
}

impl State {
    fn new(classic: bool) -> Self {
        State {
            player: Player::new(5, 25),
            frame_time: 0.0,
            obstacle: Obstacle::new(SCREEN_WIDTH, 0, classic),
            mode: GameMode::Playing,
            score: 0,
            classic_mode: classic,
            sky_time: 0.0,
            weather_state: 0,
            weather_timer: 0.0,
            clouds: Vec::new(),
            rng: RandomNumberGenerator::new(),
            #[cfg(target_arch = "wasm32")]
            game_over_reported: false,
        }
    }

    fn play(&mut self, ctx: &mut BTerm) {
        if self.classic_mode {
            ctx.cls_bg(NAVY);
        } else {
            self.sky_time += ctx.frame_time_ms;
            let sky_color = sky_bg_color_at(self.sky_time);
            ctx.cls_bg(sky_color);
            self.update_weather(ctx.frame_time_ms);
            self.draw_sky(ctx, sky_color);
            self.draw_clouds(ctx, sky_color);
        }

        self.frame_time += ctx.frame_time_ms;
        if self.frame_time > frame_duration_for(self.score, self.classic_mode) {
            self.frame_time = 0.0;
            self.player.gravity_and_move(player_x_speed_for(self.score, self.classic_mode));
        }

        if let Some(VirtualKeyCode::Space) = ctx.key {
            self.player.flap();
        }

        self.player.render(ctx, self.classic_mode);
        ctx.print(0, 0, "Press SPACE to flap.");
        ctx.print(0, 1, &format!("Score: {}", self.score));
        self.obstacle.render(ctx, self.player.x, self.classic_mode);

        if self.player.x > self.obstacle.x {
            self.score += 1;
            self.obstacle = Obstacle::new(
                self.player.x + SCREEN_WIDTH,
                self.score,
                self.classic_mode,
            );
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

    // Stub implementations — filled in later tasks
    fn update_weather(&mut self, _delta_ms: f32) {}
    fn draw_sky(&self, _ctx: &mut BTerm, _sky_color: RGB) {}
    fn draw_clouds(&self, _ctx: &mut BTerm, _sky_color: RGB) {}
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
                *self = State::new(self.classic_mode);
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

    fn render(&mut self, ctx: &mut BTerm, classic_mode: bool) {
        if classic_mode {
            ctx.set(0, self.y, YELLOW, BLACK, to_cp437('@'));
        }
        // NEW mode: cell is left as sky background; JS dragon SVG covers the position
        #[cfg(target_arch = "wasm32")]
        PLAYER_Y.with(|y| y.set(self.y));
    }

    fn gravity_and_move(&mut self, x_speed: i32) {
        if self.velocity < 2.0 {
            self.velocity += 0.5;
        }
        self.y += self.velocity as i32;
        self.x += x_speed;
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
    fn new(x: i32, score: i32, classic_mode: bool) -> Self {
        let mut random = RandomNumberGenerator::new();
        Obstacle {
            x,
            gap_y: random.range(10, 40),
            size: gap_size_for(score, classic_mode),
        }
    }

    fn render(&mut self, ctx: &mut BTerm, player_x: i32, _classic_mode: bool) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_duration_classic_is_fixed() {
        assert_eq!(frame_duration_for(0, true), 75.0);
        assert_eq!(frame_duration_for(100, true), 75.0);
    }

    #[test]
    fn frame_duration_new_decreases_with_score() {
        assert_eq!(frame_duration_for(0, false), 75.0);
        assert_eq!(frame_duration_for(30, false), 30.0);
        assert_eq!(frame_duration_for(100, false), 30.0); // capped
    }

    #[test]
    fn gap_size_classic_shrinks_every_pipe() {
        assert_eq!(gap_size_for(0, true), 20);
        assert_eq!(gap_size_for(10, true), 10);
        assert_eq!(gap_size_for(20, true), 2); // capped
        assert_eq!(gap_size_for(50, true), 2);
    }

    #[test]
    fn gap_size_new_shrinks_every_20_pipes() {
        assert_eq!(gap_size_for(0, false), 16);
        assert_eq!(gap_size_for(19, false), 16);
        assert_eq!(gap_size_for(20, false), 14);
        assert_eq!(gap_size_for(80, false), 8); // floor not yet hit; capped at 4 only at score >= 120
    }

    #[test]
    fn player_x_speed_classic_is_fixed() {
        assert_eq!(player_x_speed_for(0, true), 2);
        assert_eq!(player_x_speed_for(50, true), 2);
    }

    #[test]
    fn player_x_speed_new_increases_every_10() {
        assert_eq!(player_x_speed_for(0, false), 2);
        assert_eq!(player_x_speed_for(9, false), 2);
        assert_eq!(player_x_speed_for(10, false), 3);
        assert_eq!(player_x_speed_for(20, false), 4);
    }

    #[test]
    fn sky_color_at_dawn() {
        let c = sky_bg_color_at(0.0);
        assert!((c.r - 255.0 / 255.0).abs() < 0.01);
        assert!((c.g - 100.0 / 255.0).abs() < 0.01);
        assert!((c.b -  50.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn sky_color_at_night() {
        let c = sky_bg_color_at(SKY_PHASE_MS * 5.0); // start of night
        assert!((c.r - 10.0 / 255.0).abs() < 0.01);
        assert!((c.g - 10.0 / 255.0).abs() < 0.01);
        assert!((c.b - 55.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn sky_color_wraps_after_full_cycle() {
        let c0 = sky_bg_color_at(0.0);
        let c1 = sky_bg_color_at(SKY_TOTAL_MS);
        assert!((c0.r - c1.r).abs() < 0.01);
        assert!((c0.g - c1.g).abs() < 0.01);
        assert!((c0.b - c1.b).abs() < 0.01);
    }
}
