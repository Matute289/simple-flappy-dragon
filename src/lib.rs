use bracket_lib::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const SCREEN_WIDTH: i32 = 83;
const PLAYER_SCREEN_COL: i32 = 3; // visual column; gives left margin for dragon SVG
const SCREEN_HEIGHT: i32 = 50;
#[allow(dead_code)]
const FRAME_DURATION: f32 = 75.0;

// Universe journey phase thresholds (pipes passed)
const PHASE_CITY_END: i32  = 25;   // city zooms out and disappears
const PHASE_SKY_END: i32   = 100;  // high atmosphere / stratosphere
const PHASE_ATMO_END: i32  = 200;  // entering space
const PHASE_EARTH_END: i32 = 300;  // Earth shrinks away
const PHASE_SOLAR_END: i32 = 1300; // past Pluto (through solar system)
const PHASE_OORT_END: i32  = 1400; // WIN: past Oort cloud!

fn lerp_rgb(a: RGB, b: RGB, t: f32) -> RGB {
    RGB::from_f32(lerp(a.r, b.r, t), lerp(a.g, b.g, t), lerp(a.b, b.b, t))
}

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
    (75.0 - score as f32 * 0.5).max(40.0)
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
fn player_x_speed_for(_score: i32, classic: bool) -> i32 {
    if classic { return 2; }
    2 // speed controlled only by frame_duration — no sudden x_speed jumps
}

// --- WASM-only: JS import and restart flag ---

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    fn on_game_over(score: u32);
    fn on_game_win(score: u32);
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

#[cfg(target_arch = "wasm32")]
thread_local! {
    static FLAP_REQUESTED:   Cell<bool> = Cell::new(false);
    static BEGIN_REQUESTED:  Cell<bool> = Cell::new(false);
    static PAUSE_REQUESTED:  Cell<bool> = Cell::new(false);
    static RESUME_REQUESTED: Cell<bool> = Cell::new(false);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn flap() {
    FLAP_REQUESTED.with(|f| f.set(true));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn begin_play() {
    BEGIN_REQUESTED.with(|b| b.set(true));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn pause_game() {
    PAUSE_REQUESTED.with(|p| p.set(true));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn resume_game() {
    RESUME_REQUESTED.with(|r| r.set(true));
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
    let context = BTermBuilder::simple(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)?
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
    #[cfg(target_arch = "wasm32")]
    win_reported: bool,
}

impl State {
    fn new(classic: bool) -> Self {
        let mut rng = RandomNumberGenerator::new();
        let obstacle = Obstacle::new(SCREEN_WIDTH, 0, classic, &mut rng);
        State {
            player: Player::new(5, 25),
            frame_time: 0.0,
            obstacle,
            mode: GameMode::Waiting,
            score: 0,
            classic_mode: classic,
            sky_time: 0.0,
            weather_state: 0,
            weather_timer: 20.0,
            clouds: Vec::new(),
            rng,
            #[cfg(target_arch = "wasm32")]
            game_over_reported: false,
            #[cfg(target_arch = "wasm32")]
            win_reported: false,
        }
    }

    fn play(&mut self, ctx: &mut BTerm) {
        if self.classic_mode {
            ctx.cls_bg(NAVY);
        } else {
            if self.score >= PHASE_OORT_END {
                self.mode = GameMode::Win;
                return;
            }
            let bg_color = self.draw_background(ctx);
            if self.score < PHASE_ATMO_END {
                self.sky_time += ctx.frame_time_ms;
                self.draw_sky(ctx, bg_color);       // sun, moon, stars on top of city/sky bg
                self.update_weather(ctx.frame_time_ms);
                self.draw_clouds(ctx, bg_color);
            }
        }

        self.frame_time += ctx.frame_time_ms;
        if self.frame_time > frame_duration_for(self.score, self.classic_mode) {
            self.frame_time = 0.0;
            self.player.gravity_and_move(player_x_speed_for(self.score, self.classic_mode));
        }

        if let Some(VirtualKeyCode::Space) = ctx.key {
            self.player.flap();
        }

        #[cfg(target_arch = "wasm32")]
        if FLAP_REQUESTED.with(|f| { if f.get() { f.set(false); true } else { false } }) {
            self.player.flap();
        }

        self.player.render(ctx, self.classic_mode);
        if self.classic_mode {
            ctx.print(0, 0, "Press SPACE to flap.");
            ctx.print(0, 1, &format!("Score: {}", self.score));
        } else {
            // Hint in top-left (small, dim)
            ctx.print_color(0, 0, RGB::from_u8(180, 180, 180), RGB::from_u8(0, 0, 0), " SPACE ");
            // Score box in top-right
            let score_str = format!("{}", self.score);
            let box_x = SCREEN_WIDTH - score_str.len() as i32 - 4;
            ctx.draw_box(box_x, 0, score_str.len() as i32 + 3, 2, RGB::named(YELLOW), RGB::from_u8(0, 0, 0));
            ctx.print_color(box_x + 2, 1, RGB::named(YELLOW), RGB::from_u8(0, 0, 0), &score_str);
            // Phase label
            let phase_label = match self.score {
                s if s < PHASE_CITY_END  => "Ciudad",
                s if s < PHASE_SKY_END   => "Estratosfera",
                s if s < PHASE_ATMO_END  => "Atmosfera",
                s if s < PHASE_EARTH_END => "Orbita",
                s if s < 400             => "Marte",
                s if s < 620             => "Jupiter",
                s if s < 820             => "Saturno",
                s if s < 1020            => "Urano",
                s if s < 1200            => "Neptuno",
                s if s < PHASE_SOLAR_END => "Pluton",
                _                        => "Nube de Oort",
            };
            ctx.print_color(1, 1, RGB::from_u8(150, 150, 200), RGB::from_u8(0, 0, 0), phase_label);
        }
        self.obstacle.render(ctx, self.player.x, self.classic_mode);

        let pipe_width: i32 = if self.classic_mode { 1 } else { 2 };
        if self.player.x > self.obstacle.x + pipe_width - 1 {
            self.score += 1;
            self.obstacle = Obstacle::new(
                self.player.x + SCREEN_WIDTH,
                self.score,
                self.classic_mode,
                &mut self.rng,
            );
        }
        if self.player.y <= 0 || self.player.y > SCREEN_HEIGHT || self.obstacle.hit_obstacle(&self.player) {
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
        // Clouds only in city/sky phases; start after 5 pipes
        if self.score < 5 || self.score >= PHASE_ATMO_END {
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
        // weather_state: 0=Clear, 1=Light, 2=Cloudy, 3=Overcast, 4=Storm
        // sun/moon hidden when weather_state >= 3; dimmed when weather_state == 2
        let t = self.sky_time % SKY_TOTAL_MS;
        let phase_f = t / SKY_PHASE_MS;
        let phase = phase_f as usize % 6;
        let phase_t = phase_f - phase as f32;

        // ── Stars: phase 5 (full), phase 0 (fade out), phase 4 (fade in) ──
        const STARS: [(i32, i32); 10] = [
            (10, 2), (20, 4), (30, 2), (45, 5), (55, 3),
            (63, 6), (70, 2), (15, 7), (38, 8), (60, 7),
        ];
        let star_alpha: f32 = match phase {
            5 => 1.0,
            0 => 1.0 - phase_t,
            4 => phase_t,
            _ => 0.0,
        };
        if star_alpha > 0.05 {
            let v = (star_alpha * 200.0) as u8;
            let star_color = RGB::from_u8(v, v, (v as u32 * 2 / 3) as u8);
            for (sx, sy) in &STARS {
                ctx.set(*sx, *sy, star_color, sky_color, 250u16); // ·
            }
        }

        // ── Sun: phases 0–4 only. Enters left at dawn, exits right at end of dusk ──
        // arc span: 5 phases × 30s = 150s. x sweeps 5→74.
        // At dawn start (t=0): sun_x=5. At dusk end (t=150000): sun_x=74.
        // NEVER visible during phase 5 → no overlap with moon possible.
        if phase < 5 && self.weather_state < 3 {
            let sun_t = (t / (SKY_PHASE_MS * 5.0)).min(1.0);
            let sun_x = (5.0 + sun_t * 69.0) as i32;
            let (sun_main, sun_ray) = if self.weather_state == 2 {
                (RGB::from_u8(160, 140, 0), RGB::from_u8(100, 90, 0))
            } else {
                (RGB::named(YELLOW), RGB::from_u8(200, 190, 60))
            };
            // 5-cell cross: left/right rays + top/bottom dots + center ☼
            if sun_x > 0 { ctx.set(sun_x - 1, 3, sun_ray, sky_color, 196u16); } // ─
            ctx.set(sun_x, 2, sun_ray, sky_color, 250u16); // · above
            ctx.set(sun_x, 3, sun_main, sky_color, 15u16); // ☼ center
            ctx.set(sun_x, 4, sun_ray, sky_color, 250u16); // · below
            if sun_x + 1 < SCREEN_WIDTH { ctx.set(sun_x + 1, 3, sun_ray, sky_color, 196u16); } // ─
        }

        // ── Moon: phases 5 and 0 only. Enters left at night start, exits right at dawn end ──
        // arc span: 2 phases × 30s = 60s. x sweeps 5→74.
        // During dawn (phase 0): moon at x≈39-74, sun at x≈5-19 → NO OVERLAP.
        // During dusk (phase 4): moon NOT visible → no overlap with exiting sun.
        let moon_visible = phase == 5 || phase == 0;
        if moon_visible && self.weather_state < 3 {
            let moon_elapsed = if phase == 5 {
                phase_t * SKY_PHASE_MS
            } else {
                SKY_PHASE_MS + phase_t * SKY_PHASE_MS
            };
            let moon_t = (moon_elapsed / (SKY_PHASE_MS * 2.0)).min(1.0);
            let moon_x = (5.0 + moon_t * 69.0) as i32;
            let (moon_main, moon_dim) = if self.weather_state == 2 {
                (RGB::from_u8(150, 150, 150), RGB::from_u8(100, 100, 100))
            } else {
                (RGB::named(WHITE), RGB::from_u8(180, 180, 200))
            };
            // Moon: 2-wide circle + dot above
            ctx.set(moon_x, 3, moon_dim, sky_color, 250u16); // · above
            ctx.set(moon_x, 4, moon_main, sky_color, 9u16); // ○ left
            if moon_x + 1 < SCREEN_WIDTH {
                ctx.set(moon_x + 1, 4, moon_dim, sky_color, 9u16); // ○ right (dimmer)
            }
        }
    }
    #[allow(dead_code)]
    fn draw_ground(&self, ctx: &mut BTerm) {
        let (grass_color, earth_color) = match self.weather_state {
            4 => (RGB::from_u8(20, 80, 15), RGB::from_u8(15, 55, 10)), // storm: darker
            _ => (RGB::from_u8(40, 130, 25), RGB::from_u8(25, 85, 15)), // normal: green
        };
        for x in 0..SCREEN_WIDTH {
            ctx.set(x, SCREEN_HEIGHT - 2, grass_color, grass_color, 178u16); // ▓ grass
            ctx.set(x, SCREEN_HEIGHT - 1, earth_color, earth_color, 219u16); // █ earth
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

    fn draw_background(&self, ctx: &mut BTerm) -> RGB {
        let s = self.score;
        if s < PHASE_CITY_END {
            self.draw_city(ctx, s as f32 / PHASE_CITY_END as f32)
        } else if s < PHASE_SKY_END {
            let t = (s - PHASE_CITY_END) as f32 / (PHASE_SKY_END - PHASE_CITY_END) as f32;
            self.draw_high_sky(ctx, t)
        } else if s < PHASE_ATMO_END {
            let t = (s - PHASE_SKY_END) as f32 / (PHASE_ATMO_END - PHASE_SKY_END) as f32;
            self.draw_atmosphere(ctx, t)
        } else if s < PHASE_EARTH_END {
            let t = (s - PHASE_ATMO_END) as f32 / (PHASE_EARTH_END - PHASE_ATMO_END) as f32;
            self.draw_earth_phase(ctx, t)
        } else if s < PHASE_SOLAR_END {
            let t = (s - PHASE_EARTH_END) as f32 / (PHASE_SOLAR_END - PHASE_EARTH_END) as f32;
            self.draw_solar_system(ctx, t)
        } else {
            let t = (s - PHASE_SOLAR_END) as f32 / (PHASE_OORT_END - PHASE_SOLAR_END) as f32;
            self.draw_oort_cloud(ctx, t)
        }
    }

    fn draw_city(&self, ctx: &mut BTerm, t: f32) -> RGB {
        let sky = sky_bg_color_at(self.sky_time);
        ctx.cls_bg(sky);
        let horizon_y = (38.0 + t * 15.0) as i32;
        let scale = (1.0 - t * 1.15_f32).max(0.0);
        let buildings: &[(i32, i32, f32)] = &[
            (0,6,0.55),(6,4,0.40),(10,8,0.75),(18,3,0.35),(21,7,0.65),
            (28,4,0.45),(32,5,0.80),(37,6,0.60),(43,4,0.50),(47,7,0.70),
            (54,3,0.40),(57,8,0.85),(65,5,0.55),(70,4,0.65),(74,6,0.50),
        ];
        for &(bx, bw, bh) in buildings {
            let max_h = (bh * 16.0 * scale) as i32;
            if max_h <= 0 { continue; }
            let top_y = horizon_y - max_h;
            if top_y >= SCREEN_HEIGHT { continue; }
            let wall = RGB::from_u8(70, 70, 85);
            let edge = RGB::from_u8(55, 55, 68);
            let w_on = RGB::from_u8(240, 210, 80);
            let w_off= RGB::from_u8(25, 25, 45);
            for y in top_y.max(0)..horizon_y.min(SCREEN_HEIGHT) {
                for x in bx..(bx+bw).min(SCREEN_WIDTH) {
                    let c = if x==bx || x==bx+bw-1 { edge } else { wall };
                    ctx.set(x, y, c, c, 219u16);
                }
            }
            // windows
            let mut wy = top_y + 1;
            while wy < horizon_y - 1 {
                let mut wx = bx + 1;
                while wx < bx + bw - 1 {
                    let lit = ((wx*7 + wy*13 + bx*3) % 5) != 0;
                    let wc = if lit { w_on } else { w_off };
                    if wy >= 0 && wy < SCREEN_HEIGHT && wx < SCREEN_WIDTH {
                        ctx.set(wx, wy, wc, wall, 250u16);
                    }
                    wx += 2;
                }
                wy += 2;
            }
            // rooftop
            if top_y >= 0 && top_y < SCREEN_HEIGHT {
                for x in bx..(bx+bw).min(SCREEN_WIDTH) {
                    ctx.set(x, top_y, RGB::from_u8(90,90,105), edge, 223u16);
                }
            }
        }
        // ground
        for y in horizon_y.max(0)..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let c = if y == horizon_y { RGB::from_u8(55,55,60) } else { RGB::from_u8(35,110,25) };
                ctx.set(x, y, c, c, 219u16);
            }
        }
        sky
    }

    fn draw_high_sky(&self, ctx: &mut BTerm, t: f32) -> RGB {
        let sky = sky_bg_color_at(self.sky_time);
        ctx.cls_bg(sky);
        // Fading city silhouette at bottom
        if t < 0.4 {
            let fade = 1.0 - t / 0.4;
            let c = RGB::from_u8((50.0*fade) as u8, (50.0*fade) as u8, (60.0*fade) as u8);
            for x in 0..SCREEN_WIDTH {
                ctx.set(x, SCREEN_HEIGHT-1, c, c, 223u16);
                if t < 0.2 { ctx.set(x, SCREEN_HEIGHT-2, c, c, 220u16); }
            }
        }
        sky
    }

    fn draw_atmosphere(&self, ctx: &mut BTerm, t: f32) -> RGB {
        let day_night = sky_bg_color_at(self.sky_time);
        let space = RGB::from_u8(3, 5, 20);
        let sky = lerp_rgb(day_night, space, t);
        ctx.cls_bg(sky);
        // Curved horizon at bottom
        if t < 0.6 {
            let fade = 1.0 - t / 0.6;
            let atmo = RGB::from_u8((60.0*fade) as u8, (120.0*fade) as u8, (220.0*fade) as u8);
            for x in 0..SCREEN_WIDTH {
                let dx = (x - 40) as f32 / 40.0;
                let curve_h = (5.0 * fade * (1.0 - dx*dx).max(0.0)) as i32;
                let y = SCREEN_HEIGHT - 2 - curve_h;
                if y >= 0 && y < SCREEN_HEIGHT { ctx.set(x, y, atmo, sky, 196u16); }
            }
        }
        if t > 0.3 {
            self.draw_space_stars(ctx, sky, (t - 0.3) / 0.7);
        }
        sky
    }

    fn draw_earth_phase(&self, ctx: &mut BTerm, t: f32) -> RGB {
        let bg = RGB::from_u8(3, 5, 20);
        ctx.cls_bg(bg);
        self.draw_space_stars(ctx, bg, 1.0);
        let r = ((1.0 - t) * 13.0) as i32;
        if r > 0 { self.draw_planet_earth(ctx, bg, 62, 22, r); }
        bg
    }

    fn draw_planet_earth(&self, ctx: &mut BTerm, bg: RGB, cx: i32, cy: i32, r: i32) {
        if r <= 0 { return; }
        let land  = RGB::from_u8(34, 139, 34);
        let ocean = RGB::from_u8(30, 80, 180);
        let ice   = RGB::from_u8(220, 235, 250);
        let shade = RGB::from_u8(8, 25, 70);
        for dy in -r..=r {
            let rw = ((r*r - dy*dy) as f32).max(0.0).sqrt() as i32;
            for dx in -rw..=rw {
                let x = cx+dx; let y = cy+dy;
                if x<0||x>=SCREEN_WIDTH||y<0||y>=SCREEN_HEIGHT { continue; }
                let dtop = (dy + r) as f32 / (2*r) as f32;
                let hash = ((dx + dy*7 + cx).abs()) % 13;
                let c = if dtop < 0.12 || dtop > 0.88 { ice }
                        else if dx > rw*2/3 { shade }
                        else if hash < 5 { land } else { ocean };
                ctx.set(x, y, c, c, if r>5 {178u16} else {219u16});
            }
        }
        // atmosphere glow
        let r2 = r+1;
        for dy in -r2..=r2 {
            let rw2 = ((r2*r2 - dy*dy) as f32).max(0.0).sqrt() as i32;
            for dx in &[-rw2, rw2] {
                let x = cx+dx; let y = cy+dy;
                if x<0||x>=SCREEN_WIDTH||y<0||y>=SCREEN_HEIGHT { continue; }
                if dx*dx + dy*dy > r*r {
                    ctx.set(x, y, RGB::from_u8(80,140,220), bg, 250u16);
                }
            }
        }
    }

    fn draw_solar_system(&self, ctx: &mut BTerm, t: f32) -> RGB {
        let bg = RGB::from_u8(2, 2, 10);
        ctx.cls_bg(bg);
        self.draw_space_stars(ctx, bg, 1.0);
        // Sun (always visible, shrinks and moves left)
        let sun_x = (72.0 - t * 25.0) as i32;
        let sun_r = (6.0 - t * 3.5).max(1.0) as i32;
        self.draw_sun_glow(ctx, bg, sun_x, 8, sun_r);
        // Mars: t=0.0–0.1 (score 300–430)
        if t < 0.13 {
            let mt = t / 0.13;
            let mx = (75.0 - mt * 90.0) as i32;
            if mx > -6 && mx < SCREEN_WIDTH+6 {
                let mr = (4.0 - mt*2.0).max(1.0) as i32;
                self.draw_planet_simple(ctx, bg, mx, 28, mr, RGB::from_u8(180,80,40), RGB::from_u8(120,50,25));
                if mt < 0.4 { ctx.print_color(mx-2, 28-mr-1, RGB::from_u8(200,120,100), bg, "Marte"); }
            }
        }
        // Jupiter: t=0.1–0.4 (score 430–820)
        if t >= 0.09 && t < 0.42 {
            let jt = (t-0.09)/0.33;
            let jx = (76.0 - jt*95.0) as i32;
            if jx > -18 && jx < SCREEN_WIDTH+18 {
                let jr = (10.0 - jt*4.0).max(2.0) as i32;
                self.draw_planet_jupiter(ctx, bg, jx, 30, jr);
                if jt < 0.35 { ctx.print_color(jx-3, 30-jr-1, RGB::from_u8(210,170,120), bg, "Jupiter"); }
            }
        }
        // Saturn: t=0.28–0.58 (score 664–1054)
        if t >= 0.27 && t < 0.60 {
            let st = (t-0.27)/0.33;
            let sx = (78.0 - st*100.0) as i32;
            if sx > -22 && sx < SCREEN_WIDTH+22 {
                let sr = (8.0 - st*3.0).max(2.0) as i32;
                self.draw_planet_saturn(ctx, bg, sx, 22, sr);
                if st < 0.3 { ctx.print_color(sx-3, 22-sr-2, RGB::from_u8(220,200,140), bg, "Saturno"); }
            }
        }
        // Uranus: t=0.48–0.74
        if t >= 0.47 && t < 0.76 {
            let ut = (t-0.47)/0.29;
            let ux = (74.0 - ut*88.0) as i32;
            if ux > -10 && ux < SCREEN_WIDTH+10 {
                let ur = (5.0 - ut*2.0).max(1.0) as i32;
                self.draw_planet_simple(ctx, bg, ux, 18, ur, RGB::from_u8(100,220,220), RGB::from_u8(60,160,170));
                if ut < 0.3 { ctx.print_color(ux-2, 18-ur-1, RGB::from_u8(150,230,230), bg, "Urano"); }
            }
        }
        // Neptune: t=0.68–0.90
        if t >= 0.67 && t < 0.92 {
            let nt = (t-0.67)/0.25;
            let nx = (72.0 - nt*82.0) as i32;
            if nx > -8 && nx < SCREEN_WIDTH+8 {
                let nr = (4.0 - nt*1.5).max(1.0) as i32;
                self.draw_planet_simple(ctx, bg, nx, 25, nr, RGB::from_u8(50,80,210), RGB::from_u8(30,55,160));
                if nt < 0.3 { ctx.print_color(nx-2, 25-nr-1, RGB::from_u8(100,130,240), bg, "Neptuno"); }
            }
        }
        // Pluto: t=0.88–1.0
        if t >= 0.87 {
            let pt = (t-0.87)/0.13;
            let px = (70.0 - pt*65.0) as i32;
            if px > 0 && px < SCREEN_WIDTH {
                ctx.set(px, 20, RGB::from_u8(200,185,165), bg, 9u16);
                if pt < 0.5 { ctx.print_color(px-2, 19, RGB::from_u8(180,165,145), bg, "Pluton"); }
            }
        }
        bg
    }

    fn draw_sun_glow(&self, ctx: &mut BTerm, bg: RGB, cx: i32, cy: i32, r: i32) {
        if r <= 0 { return; }
        let core  = RGB::from_u8(255, 250, 180);
        let outer = RGB::from_u8(255, 200, 50);
        let corona= RGB::from_u8(255, 140, 20);
        for dy in -r..=r {
            let rw = ((r*r - dy*dy) as f32).max(0.0).sqrt() as i32;
            for dx in -rw..=rw {
                let x=cx+dx; let y=cy+dy;
                if x<0||x>=SCREEN_WIDTH||y<0||y>=SCREEN_HEIGHT { continue; }
                let c = if dx*dx+dy*dy < r*r*2/3 { core } else { outer };
                ctx.set(x, y, c, c, 219u16);
            }
        }
        let r2 = r+2;
        for dy in -r2..=r2 {
            let rw2 = ((r2*r2-dy*dy) as f32).max(0.0).sqrt() as i32;
            for dx in -rw2..=rw2 {
                let x=cx+dx; let y=cy+dy;
                if x<0||x>=SCREEN_WIDTH||y<0||y>=SCREEN_HEIGHT { continue; }
                if dx*dx+dy*dy > r*r && dx*dx+dy*dy <= r2*r2 {
                    ctx.set(x, y, corona, bg, 250u16);
                }
            }
        }
    }

    fn draw_planet_simple(&self, ctx: &mut BTerm, _bg: RGB, cx: i32, cy: i32, r: i32, main_c: RGB, dark_c: RGB) {
        if r <= 0 { return; }
        for dy in -r..=r {
            let rw = ((r*r-dy*dy) as f32).max(0.0).sqrt() as i32;
            for dx in -rw..=rw {
                let x=cx+dx; let y=cy+dy;
                if x<0||x>=SCREEN_WIDTH||y<0||y>=SCREEN_HEIGHT { continue; }
                ctx.set(x, y, if dx > rw/2 { dark_c } else { main_c }, if dx > rw/2 { dark_c } else { main_c }, 219u16);
            }
        }
    }

    fn draw_planet_jupiter(&self, ctx: &mut BTerm, _bg: RGB, cx: i32, cy: i32, r: i32) {
        if r <= 0 { return; }
        let bands: &[RGB] = &[
            RGB::from_u8(200,150,100), RGB::from_u8(155,105,65),
            RGB::from_u8(220,178,128), RGB::from_u8(190,138,92),
            RGB::from_u8(212,163,108),
        ];
        for dy in -r..=r {
            let rw = ((r*r-dy*dy) as f32).max(0.0).sqrt() as i32;
            let bi = ((dy+r) as usize * bands.len()) / (2*r as usize+1);
            let bc = bands[bi.min(bands.len()-1)];
            let dark = RGB::from_f32(bc.r*0.65, bc.g*0.65, bc.b*0.65);
            for dx in -rw..=rw {
                let x=cx+dx; let y=cy+dy;
                if x<0||x>=SCREEN_WIDTH||y<0||y>=SCREEN_HEIGHT { continue; }
                ctx.set(x, y, if dx > rw*2/3 { dark } else { bc }, if dx > rw*2/3 { dark } else { bc }, 219u16);
            }
        }
        // Great Red Spot
        if r > 5 {
            let sx=cx-r/3; let sy=cy+r/4;
            for dy in -2i32..=2 { for dx in -4i32..=4 {
                let x=sx+dx; let y=sy+dy;
                if x>=0&&x<SCREEN_WIDTH&&y>=0&&y<SCREEN_HEIGHT {
                    ctx.set(x,y,RGB::from_u8(200,75,55),RGB::from_u8(200,75,55),178u16);
                }
            }}
        }
    }

    fn draw_planet_saturn(&self, ctx: &mut BTerm, bg: RGB, cx: i32, cy: i32, r: i32) {
        if r <= 0 { return; }
        let ring1 = RGB::from_u8(180,158,118);
        let ring2 = RGB::from_u8(140,120,88);
        // rings (behind planet — draw first)
        for dx in -(r*3)..(r*3) {
            let x = cx+dx;
            if x<0||x>=SCREEN_WIDTH { continue; }
            let adx = dx.abs();
            if adx > r+1 && adx < r*3 {
                let rc = if dx%3==0 { ring2 } else { ring1 };
                for ry in [cy-1, cy] {
                    if ry>=0&&ry<SCREEN_HEIGHT { ctx.set(x, ry, rc, bg, 196u16); }
                }
            }
        }
        let body = RGB::from_u8(220,200,140);
        let dark = RGB::from_u8(155,138,88);
        for dy in -r..=r {
            let rw = ((r*r-dy*dy) as f32).max(0.0).sqrt() as i32;
            for dx in -rw..=rw {
                let x=cx+dx; let y=cy+dy;
                if x<0||x>=SCREEN_WIDTH||y<0||y>=SCREEN_HEIGHT { continue; }
                ctx.set(x,y, if dx>rw*2/3{dark}else{body}, if dx>rw*2/3{dark}else{body}, 219u16);
            }
        }
    }

    fn draw_oort_cloud(&self, ctx: &mut BTerm, t: f32) -> RGB {
        let bg = RGB::from_u8(1, 1, 8);
        ctx.cls_bg(bg);
        self.draw_space_stars(ctx, bg, 1.0);
        // Scattered ice particles
        let count = (t * 120.0) as i32;
        for i in 0..count.min(120) {
            let x = ((i*17 + i*i*3) % (SCREEN_WIDTH as i32)).abs();
            let y = ((i*11 + i*i*7) % (SCREEN_HEIGHT as i32)).abs();
            let b = ((i*31)%80) as u8 + 40;
            ctx.set(x, y, RGB::from_u8(b/2,b/2,b), bg, 250u16);
        }
        if t > 0.2 {
            ctx.print_color(22, 5, RGB::from_u8(140,140,200), bg, "~ Nube de Oort ~");
        }
        bg
    }

    fn draw_space_stars(&self, ctx: &mut BTerm, bg: RGB, alpha: f32) {
        const STARS: [(i32,i32,u8); 40] = [
            (5,3,200),(12,7,150),(22,2,255),(31,9,180),(40,4,220),
            (48,11,160),(58,6,200),(67,3,240),(73,8,170),(2,14,190),
            (15,17,210),(25,12,155),(35,19,230),(44,15,175),(55,13,205),
            (64,18,185),(71,14,220),(8,22,165),(19,25,215),(29,20,195),
            (38,27,175),(50,24,240),(60,21,155),(69,26,200),(3,30,210),
            (14,33,170),(24,28,195),(34,35,180),(46,32,225),(56,37,165),
            (66,30,200),(76,34,190),(9,40,215),(20,43,155),(32,38,235),
            (42,45,175),(53,41,190),(63,47,210),(72,42,165),(78,48,195),
        ];
        for &(x,y,b) in &STARS {
            let v = (b as f32 * alpha) as u8;
            if v < 15 { continue; }
            let glyph = if b > 220 { 15u16 } else { 250u16 };
            ctx.set(x, y, RGB::from_u8(v,v,(v as u32*9/10) as u8), bg, glyph);
        }
    }

    fn celebrate(&mut self, ctx: &mut BTerm) {
        #[cfg(target_arch = "wasm32")]
        if !self.win_reported {
            on_game_win(self.score as u32);
            self.win_reported = true;
        }
        let bg = RGB::from_u8(1, 1, 10);
        ctx.cls_bg(bg);
        self.draw_space_stars(ctx, bg, 1.0);
        ctx.print_color(12, 16, RGB::named(YELLOW), bg, "*** FELICITACIONES, GANASTE! ***");
        ctx.print_color(18, 18, RGB::named(WHITE), bg, "Te fuiste del sistema solar");
        ctx.print_color(15, 20, RGB::from_u8(150,200,255), bg, "Pasaste la Nube de Oort!");
        ctx.print_color(20, 22, RGB::from_u8(180,180,255), bg, &format!("Pipes: {}", self.score));
    }

    fn wait(&mut self, ctx: &mut BTerm) {
        if self.classic_mode {
            ctx.cls_bg(NAVY);
        } else {
            let bg_color = self.draw_background(ctx);
            if self.score < PHASE_ATMO_END {
                self.draw_sky(ctx, bg_color);
                self.draw_clouds(ctx, bg_color);
            }
        }

        // Expose player position so the dragon SVG overlay sits correctly during wait
        #[cfg(target_arch = "wasm32")]
        PLAYER_Y.with(|y| y.set(self.player.y));

        #[cfg(target_arch = "wasm32")]
        if BEGIN_REQUESTED.with(|b| { if b.get() { b.set(false); true } else { false } }) {
            self.mode = GameMode::Playing;
        }

        // Discard any flap input accumulated while waiting — prevents a phantom flap
        // on the first tick after transitioning to Playing.
        #[cfg(target_arch = "wasm32")]
        FLAP_REQUESTED.with(|f| f.set(false));
    }

    fn show_paused(&mut self, ctx: &mut BTerm) {
        if self.classic_mode {
            ctx.cls_bg(NAVY);
        } else {
            let bg_color = self.draw_background(ctx);
            if self.score < PHASE_ATMO_END {
                self.draw_sky(ctx, bg_color);
                self.draw_clouds(ctx, bg_color);
            }
        }
        self.obstacle.render(ctx, self.player.x, self.classic_mode);
        self.player.render(ctx, self.classic_mode);

        // On native: keyboard can resume. On WASM, JS handles it via RESUME_REQUESTED
        // (using ctx.key here on WASM causes the same ESC that triggered the pause to
        // immediately un-pause within the same tick).
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(k) = ctx.key {
            if matches!(k, VirtualKeyCode::Space | VirtualKeyCode::Escape) {
                self.mode = GameMode::Playing;
            }
        }

        // Discard any flap input accumulated while paused — prevents a phantom flap
        // on the first tick after resuming to Playing.
        #[cfg(target_arch = "wasm32")]
        FLAP_REQUESTED.with(|f| f.set(false));
    }
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        #[cfg(target_arch = "wasm32")]
        {
            let restart = RESTART_REQUESTED.with(|r| {
                if r.get() { r.set(false); true } else { false }
            });
            if restart {
                let new_classic = CLASSIC_MODE.with(|m| m.get());
                *self = State::new(new_classic);
                return;
            }

            // Pause/resume transitions — only affect Playing and Paused states
            if PAUSE_REQUESTED.with(|p| { if p.get() { p.set(false); true } else { false } }) {
                if matches!(self.mode, GameMode::Playing) {
                    self.mode = GameMode::Paused;
                }
            }
            if RESUME_REQUESTED.with(|r| { if r.get() { r.set(false); true } else { false } }) {
                if matches!(self.mode, GameMode::Paused) {
                    self.mode = GameMode::Playing;
                }
            }
        }

        match self.mode {
            GameMode::Waiting => self.wait(ctx),
            GameMode::Playing => self.play(ctx),
            GameMode::Paused  => self.show_paused(ctx),
            GameMode::End     => self.dead(ctx),
            GameMode::Win     => self.celebrate(ctx),
        }
    }
}

enum GameMode {
    Waiting,
    Playing,
    Paused,
    End,
    Win,
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
            ctx.set(PLAYER_SCREEN_COL, self.y, YELLOW, BLACK, to_cp437('@'));
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
        let screen_x = self.x - player_x + PLAYER_SCREEN_COL;
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
        assert_eq!(frame_duration_for(70, false), 40.0); // cap reached at 70, not 30
        assert_eq!(frame_duration_for(100, false), 40.0);
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
    fn player_x_speed_new_is_fixed() { // rename from player_x_speed_new_increases_every_10
        assert_eq!(player_x_speed_for(0, false), 2);
        assert_eq!(player_x_speed_for(9, false), 2);
        assert_eq!(player_x_speed_for(10, false), 2); // no longer increases at score 10
        assert_eq!(player_x_speed_for(20, false), 2);
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
