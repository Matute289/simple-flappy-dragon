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
    static CLASSIC_MODE: Cell<bool> = Cell::new(false);
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
    CLASSIC_MODE.with(|m| m.set(classic));
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
        let mut rng = RandomNumberGenerator::new();
        let obstacle = Obstacle::new(SCREEN_WIDTH, 0, classic, &mut rng);
        State {
            player: Player::new(5, 25),
            frame_time: 0.0,
            obstacle,
            mode: GameMode::Playing,
            score: 0,
            classic_mode: classic,
            sky_time: 0.0,
            weather_state: 0,
            weather_timer: 20.0,
            clouds: Vec::new(),
            rng,
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
                &mut self.rng,
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

    fn update_weather(&mut self, delta_ms: f32) {
        // Only active after 1 full sky cycle
        if self.sky_time < CLOUD_ACTIVATION_MS {
            return;
        }

        // Count down and transition
        self.weather_timer -= delta_ms / 1000.0;
        if self.weather_timer <= 0.0 {
            self.weather_state = self.next_weather_state();
            self.weather_timer = 15.0 + self.rng.range(0, 30) as f32;
        }

        // Drift existing clouds left
        let drift = 0.003 * delta_ms;
        for cloud in &mut self.clouds {
            cloud.x -= drift;
        }
        self.clouds.retain(|c| c.x > -(c.width as f32 + 2.0));

        // Spawn clouds to match target density
        let target: usize = match self.weather_state {
            0 => 0,
            1 => 3,
            2 => 6,
            3 => 10,
            4 => 8,
            _ => 0,
        };
        // Spawn at most one cloud per frame to avoid sudden appearance
        if self.clouds.len() < target {
            let y = self.rng.range(3, 10);
            let width = match self.weather_state {
                1 => self.rng.range(4, 9),
                2 => self.rng.range(8, 16),
                3 | 4 => self.rng.range(12, 22),
                _ => 5,
            };
            self.clouds.push(Cloud {
                x: SCREEN_WIDTH as f32 + self.rng.range(0, 20) as f32,
                y,
                width,
            });
        }
    }

    fn next_weather_state(&mut self) -> u8 {
        match self.weather_state {
            0 => if self.rng.range(0, 10) < 7 { 1 } else { 0 },
            1 => {
                let r = self.rng.range(0, 10);
                if r < 4 { 0 } else if r < 7 { 1 } else { 2 }
            }
            2 => {
                let r = self.rng.range(0, 10);
                if r < 3 { 1 } else if r < 6 { 2 } else { 3 }
            }
            3 => {
                let r = self.rng.range(0, 10);
                if r < 4 { 2 } else if r < 7 { 3 } else { 4 }
            }
            4 => {
                if self.rng.range(0, 10) < 6 { 3 } else { 2 }
            }
            _ => 0,
        }
    }

    fn draw_sky(&self, ctx: &mut BTerm, sky_color: RGB) {
        let t = self.sky_time % SKY_TOTAL_MS;
        let phase_f = t / SKY_PHASE_MS;
        let phase = phase_f as usize % 6;
        let phase_t = phase_f - phase as f32; // 0.0–1.0 within current phase

        // weather_state: 0=Clear, 1=Light, 2=Cloudy, 3=Overcast, 4=Storm
        // sun/moon hidden when weather_state >= 3; dimmed when weather_state == 2

        // ── Stars: visible in phases 5 (Night) and 0 (Dawn, fading out) ──
        const STARS: [(i32, i32); 10] = [
            (10, 2), (20, 4), (30, 2), (45, 5), (55, 3),
            (63, 6), (70, 2), (15, 7), (38, 8), (60, 7),
        ];
        let star_alpha: f32 = match phase {
            5 => 1.0,
            0 => 1.0 - phase_t, // fade out as dawn brightens
            4 => phase_t,        // fade in as dusk darkens
            _ => 0.0,
        };
        if star_alpha > 0.05 {
            let v = (star_alpha * 200.0) as u8;
            let star_color = RGB::from_u8(v, v, (v as u32 * 2 / 3) as u8); // ~66% blue keeps warm tint at all brightness levels
            for (sx, sy) in &STARS {
                ctx.set(*sx, *sy, star_color, sky_color, 250u16); // · dot
            }
        }

        // ── Sun: phases 0–4 (Dawn through Dusk) ──
        // Sun x sweeps 5→74 over 5 phases (150_000 ms)
        if phase < 5 && self.weather_state < 3 {
            let sun_progress = (t / (SKY_PHASE_MS * 5.0)).min(1.0);
            let sun_x = (5.0 + sun_progress * 69.0) as i32;
            let sun_color = if self.weather_state == 2 {
                RGB::from_u8(160, 140, 0)
            } else {
                RGB::named(YELLOW)
            };
            ctx.set(sun_x, 3, sun_color, sky_color, 15u16); // ☼
        }

        // ── Moon: phases 4–5–0 (Dusk through Night into next Dawn) ──
        // Moon visible during dusk (phase 4), night (phase 5), dawn (phase 0 of next cycle)
        let moon_visible = phase >= 4 || phase == 0;
        if moon_visible && self.weather_state < 3 {
            let moon_arc_start = SKY_PHASE_MS * 4.0;
            let moon_elapsed = if t >= moon_arc_start {
                t - moon_arc_start
            } else {
                (SKY_TOTAL_MS - moon_arc_start) + t // wrapped past midnight
            };
            let moon_t = (moon_elapsed / (SKY_PHASE_MS * 3.0)).min(1.0);
            let moon_x = (5.0 + moon_t * 69.0) as i32;
            let moon_color = if self.weather_state == 2 {
                RGB::from_u8(150, 150, 150)
            } else {
                RGB::named(WHITE)
            };
            ctx.set(moon_x, 5, moon_color, sky_color, 9u16); // ○
        }
    }
    fn draw_clouds(&self, ctx: &mut BTerm, sky_color: RGB) {
        'cloud_loop: for cloud in &self.clouds {
            let (cloud_glyph, cloud_fg): (u16, RGB) = match self.weather_state {
                1 => (176, RGB::from_u8(220, 220, 220)), // ░ light
                2 => (177, RGB::from_u8(190, 190, 190)), // ▒ medium
                3 => (178, RGB::from_u8(140, 140, 140)), // ▓ heavy
                4 => (219, RGB::from_u8(70, 70, 80)),    // █ storm
                _ => continue 'cloud_loop,
            };
            let cx = cloud.x as i32;
            for dx in 0..cloud.width {
                let x = cx + dx;
                if x < 0 || x >= SCREEN_WIDTH { continue; }
                ctx.set(x, cloud.y,     cloud_fg, sky_color, cloud_glyph);
                if cloud.y + 1 < SCREEN_HEIGHT {
                    ctx.set(x, cloud.y + 1, cloud_fg, sky_color, cloud_glyph);
                }
            }
            // Rain: only in storm state, columns every 2 chars
            if self.weather_state == 4 {
                for dx in (0..cloud.width as usize).step_by(2) {
                    let x = cx + dx as i32;
                    if x < 0 || x >= SCREEN_WIDTH { continue; }
                    for dy in 2i32..10 {
                        let ry = cloud.y + dy;
                        if ry >= SCREEN_HEIGHT { break; }
                        ctx.set(x, ry, RGB::named(CYAN), sky_color, to_cp437('|'));
                    }
                }
            }
        }
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
                let new_classic = CLASSIC_MODE.with(|m| m.get());
                *self = State::new(new_classic);
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
    classic_mode: bool,
}

impl Obstacle {
    fn new(x: i32, score: i32, classic_mode: bool, rng: &mut RandomNumberGenerator) -> Self {
        Obstacle {
            x,
            gap_y: rng.range(10, 40),
            size: gap_size_for(score, classic_mode),
            classic_mode,
        }
    }

    fn render(&mut self, ctx: &mut BTerm, player_x: i32, classic_mode: bool) {
        let screen_x = self.x - player_x;
        let half_size = self.size / 2;
        let gap_top = self.gap_y - half_size;
        let gap_bot = self.gap_y + half_size;

        if classic_mode {
            for y in 0..gap_top {
                ctx.set(screen_x, y, RED, BLACK, to_cp437('|'));
            }
            for y in gap_bot..SCREEN_HEIGHT {
                ctx.set(screen_x, y, RED, BLACK, to_cp437('|'));
            }
        } else {
            let body_dark = RGB::from_u8(30, 100, 30);
            let body_light = RGB::from_u8(60, 160, 60);
            let cap_color = RGB::from_u8(80, 190, 80);

            // Top pipe body
            for y in 0..gap_top.saturating_sub(1) {
                ctx.set(screen_x,     y, body_dark,  BLACK, 219u16); // █
                if screen_x + 1 < SCREEN_WIDTH {
                    ctx.set(screen_x + 1, y, body_light, BLACK, 221u16); // ▌ edge highlight
                }
            }
            // Top pipe bottom cap
            if gap_top > 0 {
                ctx.set(screen_x,     gap_top - 1, cap_color, BLACK, 220u16); // ▄
                if screen_x + 1 < SCREEN_WIDTH {
                    ctx.set(screen_x + 1, gap_top - 1, cap_color, BLACK, 220u16);
                }
            }
            // Bottom pipe top cap
            if gap_bot < SCREEN_HEIGHT {
                ctx.set(screen_x,     gap_bot, cap_color, BLACK, 223u16); // ▀
                if screen_x + 1 < SCREEN_WIDTH {
                    ctx.set(screen_x + 1, gap_bot, cap_color, BLACK, 223u16);
                }
            }
            // Bottom pipe body
            for y in gap_bot + 1..SCREEN_HEIGHT {
                ctx.set(screen_x,     y, body_dark,  BLACK, 219u16);
                if screen_x + 1 < SCREEN_WIDTH {
                    ctx.set(screen_x + 1, y, body_light, BLACK, 221u16);
                }
            }
        }
    }

    fn hit_obstacle(&self, player: &Player) -> bool {
        let half_size = self.size / 2;
        let pipe_width = if self.classic_mode { 1 } else { 2 };
        // Range check: player passed through the pipe this frame
        let x_match = player.x >= self.x && player.x < self.x + pipe_width;
        let above_gap = player.y < self.gap_y - half_size;
        let below_gap = player.y > self.gap_y + half_size;
        x_match && (above_gap || below_gap)
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

    #[test]
    fn hit_obstacle_classic_is_1_wide() {
        // In classic mode, only self.x matches — self.x+1 does NOT kill player
        let obs = Obstacle { x: 10, gap_y: 25, size: 10, classic_mode: true };
        let player_at_x = Player { x: 10, y: 5, velocity: 0.0 };   // above gap → hit
        let player_adj  = Player { x: 11, y: 5, velocity: 0.0 };   // x+1 → no hit
        assert!( obs.hit_obstacle(&player_at_x));
        assert!(!obs.hit_obstacle(&player_adj));
    }

    #[test]
    fn hit_obstacle_new_is_2_wide() {
        // In new mode, both self.x and self.x+1 kill the player
        let obs = Obstacle { x: 10, gap_y: 25, size: 10, classic_mode: false };
        let player_at_x   = Player { x: 10, y: 5, velocity: 0.0 };
        let player_at_x1  = Player { x: 11, y: 5, velocity: 0.0 };
        let player_at_x2  = Player { x: 12, y: 5, velocity: 0.0 };
        assert!( obs.hit_obstacle(&player_at_x));
        assert!( obs.hit_obstacle(&player_at_x1));
        assert!(!obs.hit_obstacle(&player_at_x2));
    }
}
