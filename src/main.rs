use ggez::{Context, ContextBuilder, GameResult};
use ggez::graphics::{self, Color, Text, DrawParam, Rect};
use ggez::event::{self, EventHandler};
use ggez::input::keyboard::{KeyCode, KeyInput};
use ggez::conf;
use glam::Vec2;
use std::time::Instant;
use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};

const BLOCK_SIZE: f32 = 50.0;
const GAME_WIDTH: i32 = 15;
const PLATFORM_BLOCKS: i32 = 5;
const MOVE_INTERVAL: f32 = 0.2;
const SPEED_INCREASE: f32 = 0.92; // Decrease interval

#[derive(Clone)]
struct GridBlock {
    active: bool,
    landed: bool,
    level: i32,
    falling: bool,
    fall_offset: f32,
}

#[derive(PartialEq)]
enum GameState {
    Menu,
    Playing,
    GameOver(Instant),  // Add timestamp for game over state
    Settings,
}

#[derive(Serialize, Deserialize)]
struct GameStats {
    high_score: i32,
    games_played: i32,
}

impl GameStats {
    fn new() -> Self {
        Self {
            high_score: 0,
            games_played: 0,
        }
    }

    fn load() -> Self {
        let path = Path::new("game_stats.json");
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(stats) = serde_json::from_str(&contents) {
                    return stats;
                }
            }
        }
        Self::new()
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write("game_stats.json", json)?;
        Ok(())
    }
}

struct GameData {
    state: GameState,
    grid: Vec<Vec<GridBlock>>,
    platform_position: i32,
    platform_width: i32,
    move_right: bool,
    move_timer: f32,
    move_interval: f32,
    level: i32,
    camera_offset_y: f32,
    window_width: f32,
    window_height: f32,
    current_row: i32,
    moving_platform_pos: i32,
    stats: GameStats,
}

impl GameData {
    fn new(ctx: &Context) -> Self {
        let window_width = ctx.gfx.window().inner_size().width as f32;
        let window_height = ctx.gfx.window().inner_size().height as f32;
        let platform_pos = (GAME_WIDTH - PLATFORM_BLOCKS) / 2;
        
        Self {
            state: GameState::Menu,
            grid: Vec::new(),
            platform_position: platform_pos,
            platform_width: PLATFORM_BLOCKS,
            move_right: true,
            move_timer: 0.0,
            move_interval: MOVE_INTERVAL,
            level: 0,
            camera_offset_y: 0.0,
            window_width,
            window_height,
            current_row: 0,
            moving_platform_pos: platform_pos,
            stats: GameStats::load(),
        }
    }

    fn reset_game(&mut self) {
        self.grid.clear();
        self.platform_width = PLATFORM_BLOCKS;
        self.platform_position = (GAME_WIDTH - self.platform_width) / 2;
        self.moving_platform_pos = self.platform_position;
        self.current_row = 0;
        self.move_interval = MOVE_INTERVAL;
        self.add_base_row();
        self.add_new_row();
    }

    fn add_base_row(&mut self) {
        let mut row = vec![GridBlock { 
            active: false, 
            landed: false, 
            level: 0,
            falling: false,
            fall_offset: 0.0,
        }; GAME_WIDTH as usize];
        
        for i in 0..self.platform_width {
            let pos = (self.platform_position + i) as usize;
            if pos < GAME_WIDTH as usize {
                row[pos].active = true;
                row[pos].landed = true;
                row[pos].level = 0;
            }
        }
        self.grid.push(row);
    }

    fn add_new_row(&mut self) {
        let mut row = vec![GridBlock { 
            active: false, 
            landed: false, 
            level: self.level,
            falling: false,
            fall_offset: 0.0,
        }; GAME_WIDTH as usize];
        
        for i in 0..self.platform_width {
            let pos = (self.moving_platform_pos + i) as usize;
            if pos < GAME_WIDTH as usize {
                row[pos].active = true;
            }
        }
        
        self.grid.push(row);
    }

    fn start_game(&mut self) {
        self.state = GameState::Playing;
        self.level = 0;
        self.camera_offset_y = 0.0;
        self.stats.games_played += 1;
        if let Err(e) = self.stats.save() {
            println!("Failed to save game stats: {}", e);
        }
        self.reset_game();
    }

    fn update_movement(&mut self, dt: f32) -> bool {
        // Update falling blocks
        if let Some(current_row) = self.grid.last_mut() {
            for block in current_row.iter_mut() {
                if block.falling {
                    block.fall_offset += 500.0 * dt; // Adjust speed as needed
                }
            }
        }

        self.move_timer += dt;
        if self.move_timer >= self.move_interval {
            self.move_timer = 0.0;
            
            if self.move_right {
                if self.moving_platform_pos + self.platform_width < GAME_WIDTH {
                    // Turn off leftmost block and turn on new rightmost block
                    if let Some(row) = self.grid.last_mut() {
                        // First ensure the previous block is off
                        row[self.moving_platform_pos as usize].active = false;
                        // Then move position
                        self.moving_platform_pos += 1;
                        // Finally activate new block
                        let new_pos = (self.moving_platform_pos + self.platform_width - 1) as usize;
                        if new_pos < GAME_WIDTH as usize {
                            row[new_pos].active = true;
                        }
                    }
                } else {
                    self.move_right = false;
                }
            } else {
                if self.moving_platform_pos > 0 {
                    // Turn off rightmost block and turn on new leftmost block
                    if let Some(row) = self.grid.last_mut() {
                        // First ensure the previous block is off
                        let old_pos = (self.moving_platform_pos + self.platform_width - 1) as usize;
                        if old_pos < GAME_WIDTH as usize {
                            row[old_pos].active = false;
                        }
                        // Then move position
                        self.moving_platform_pos -= 1;
                        // Finally activate new block
                        row[self.moving_platform_pos as usize].active = true;
                    }
                } else {
                    self.move_right = true;
                }
            }
            return true;
        }
        false
    }

    fn check_landing(&mut self) -> bool {
        if let Some(current_row) = self.grid.last_mut() {
            let mut active_count = 0;
            let platform_start = self.platform_position;
            let platform_end = self.platform_position + self.platform_width - 1;
            
            // First, mark blocks that should fall
            for i in 0..GAME_WIDTH as usize {
                if current_row[i].active && !current_row[i].landed {
                    if i < platform_start as usize || i > platform_end as usize {
                        current_row[i].falling = true;
                        current_row[i].fall_offset = 0.0;
                    }
                }
            }

            // Then count remaining active blocks and mark them as landed
            for i in platform_start..=platform_end {
                if i >= 0 && i < GAME_WIDTH && current_row[i as usize].active {
                    current_row[i as usize].landed = true;
                    active_count += 1;
                }
            }

            // Update platform width based on successful landing
            if active_count > 0 {
                self.platform_width = active_count;
                self.platform_position = (GAME_WIDTH - self.platform_width) / 2;
                self.moving_platform_pos = self.platform_position;
                self.current_row += 1;
                return true;
            }
        }
        false
    }

    fn handle_window_resize(&mut self, ctx: &Context) {
        self.window_width = ctx.gfx.window().inner_size().width as f32;
        self.window_height = ctx.gfx.window().inner_size().height as f32;
    }

    fn get_platform_color(&self, level: i32) -> Color {
        if level % 2 == 0 {
            Color::new(0.0, 0.0, 1.0, 1.0)  // Blue
        } else {
            Color::new(1.0, 1.0, 0.0, 1.0)  // Yellow
        }
    }
}

impl EventHandler for GameData {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        match self.state {
            GameState::Playing => {
                self.update_movement(ctx.time.delta().as_secs_f32());
            }
            GameState::GameOver(start_time) => {
                if start_time.elapsed().as_secs() >= 3 {
                    self.state = GameState::Menu;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas = graphics::Canvas::from_frame(ctx, Color::BLACK);
        let block_draw_size = self.window_width / GAME_WIDTH as f32;
        
        // Calculate the position of the moving platform (last row)
        if (self.state == GameState::Playing || matches!(self.state, GameState::GameOver(_))) 
            && self.grid.len() > 1 && self.level >= 4 {
            let moving_platform_y = self.window_height - ((self.grid.len()) as f32 * block_draw_size);
            if moving_platform_y < self.window_height * 0.7 {
                self.camera_offset_y = moving_platform_y - self.window_height * 0.3;
            }
        }

        // Set up two coordinate systems: one for the fixed base and one for the scrolling content
        let grid_start_x = (self.window_width - (GAME_WIDTH as f32 * block_draw_size)) / 2.0;

        match self.state {
            GameState::Menu => {
                canvas.set_screen_coordinates(Rect::new(
                    0.0,
                    0.0,
                    self.window_width,
                    self.window_height,
                ));

                let menu_text = vec![
                    ("Start", self.window_height / 2.0 - 60.0),
                    ("Settings", self.window_height / 2.0),
                    ("Quit", self.window_height / 2.0 + 60.0),
                ];

                for (text, y) in menu_text {
                    let text = Text::new(text);
                    let pos = Vec2::new(self.window_width / 2.0 - 50.0, y);
                    canvas.draw(&text, DrawParam::default().dest(pos).color(Color::WHITE));
                }

                // Draw stats
                let stats_text = vec![
                    format!("High Score: {}", self.stats.high_score),
                    format!("Games Played: {}", self.stats.games_played),
                ];

                for (i, text) in stats_text.iter().enumerate() {
                    let text = Text::new(text);
                    let pos = Vec2::new(
                        self.window_width / 2.0 - 50.0,
                        self.window_height / 2.0 + 120.0 + i as f32 * 30.0
                    );
                    canvas.draw(&text, DrawParam::default().dest(pos).color(Color::WHITE));
                }

                // Draw last level if it exists
                if self.level > 0 {
                    let level_text = Text::new(format!("Last Level: {}", self.level));
                    canvas.draw(
                        &level_text,
                        DrawParam::default()
                            .dest(Vec2::new(self.window_width / 2.0 - 50.0, self.window_height / 2.0 + 180.0))
                            .color(Color::WHITE),
                    );
                }
            }
            GameState::Playing | GameState::GameOver(_) => {
                // First draw the fixed base platform with no camera offset
                canvas.set_screen_coordinates(Rect::new(
                    0.0,
                    0.0,
                    self.window_width,
                    self.window_height,
                ));

                if let Some(base_row) = self.grid.first() {
                    let y = self.window_height - block_draw_size;
                    canvas.draw(
                        &graphics::Mesh::new_rectangle(
                            ctx,
                            graphics::DrawMode::fill(),
                            Rect::new(
                                grid_start_x + (self.platform_position as f32 * block_draw_size),
                                y,
                                block_draw_size * self.platform_width as f32,
                                block_draw_size,
                            ),
                            self.get_platform_color(0),
                        )?,
                        DrawParam::default(),
                    );
                }

                // Then draw the moving blocks with camera offset
                canvas.set_screen_coordinates(Rect::new(
                    0.0,
                    self.camera_offset_y,
                    self.window_width,
                    self.window_height,
                ));

                // Draw all rows except the base
                for (row_idx, row) in self.grid.iter().enumerate().skip(1) {
                    let y = self.window_height - ((row_idx + 1) as f32 * block_draw_size);
                    
                    // Draw blocks
                    for (col_idx, block) in row.iter().enumerate() {
                        if block.active || block.falling {
                            let mut color = if block.landed { 
                                self.get_platform_color(block.level)
                            } else if block.falling {
                                Color::RED
                            } else { 
                                Color::GREEN 
                            };

                            // Make falling blocks fade out
                            if block.falling {
                                color.a = (1.0 - (block.fall_offset / (self.window_height / 2.0))).max(0.0);
                            }

                            let block_y = if block.falling {
                                y + block.fall_offset
                            } else {
                                y
                            };

                            canvas.draw(
                                &graphics::Mesh::new_rectangle(
                                    ctx,
                                    graphics::DrawMode::fill(),
                                    Rect::new(
                                        grid_start_x + (col_idx as f32 * block_draw_size),
                                        block_y,
                                        block_draw_size,
                                        block_draw_size,
                                    ),
                                    color,
                                )?,
                                DrawParam::default(),
                            );
                        }
                    }
                }

                // Draw level text with fixed position relative to view
                canvas.set_screen_coordinates(Rect::new(
                    0.0,
                    0.0,
                    self.window_width,
                    self.window_height,
                ));
                
                // Draw large level counter in the center top of the screen
                let level_text = Text::new(format!("Level {}", self.level));
                let text_scale = 2.0;
                canvas.draw(
                    &level_text,
                    DrawParam::default()
                        .dest(Vec2::new(self.window_width / 2.0 - 50.0, 30.0))
                        .scale(Vec2::new(text_scale, text_scale))
                        .color(Color::WHITE),
                );

                // If in game over state, draw "Game Over" text
                if matches!(self.state, GameState::GameOver(_)) {
                    let game_over_text = Text::new("Game Over!");
                    let text_scale = 3.0;
                    canvas.draw(
                        &game_over_text,
                        DrawParam::default()
                            .dest(Vec2::new(self.window_width / 2.0 - 100.0, self.window_height / 2.0))
                            .scale(Vec2::new(text_scale, text_scale))
                            .color(Color::RED),
                    );
                }
            }
            GameState::Settings => {
                canvas.set_screen_coordinates(Rect::new(
                    0.0,
                    0.0,
                    self.window_width,
                    self.window_height,
                ));

                let text = Text::new("Settings (Press Esc to return)");
                canvas.draw(
                    &text,
                    DrawParam::default()
                        .dest(Vec2::new(self.window_width / 2.0 - 100.0, self.window_height / 2.0))
                        .color(Color::WHITE),
                );
            }
        }

        canvas.finish(ctx)?;
        Ok(())
    }

    fn key_down_event(&mut self, _ctx: &mut Context, input: KeyInput, _repeat: bool) -> GameResult {
        match self.state {
            GameState::Menu => {
                match input.keycode {
                    Some(KeyCode::Return) => self.start_game(),
                    Some(KeyCode::S) => self.state = GameState::Settings,
                    Some(KeyCode::Q) => std::process::exit(0),
                    _ => {}
                }
            }
            GameState::Playing => {
                if let Some(KeyCode::Space) = input.keycode {
                    if self.check_landing() {
                        self.add_new_row();
                        self.level += 1;
                        self.move_interval *= SPEED_INCREASE;
                    } else {
                        // Update high score when game ends
                        if self.level > self.stats.high_score {
                            self.stats.high_score = self.level;
                            if let Err(e) = self.stats.save() {
                                println!("Failed to save high score: {}", e);
                            }
                        }
                        self.state = GameState::GameOver(Instant::now());
                    }
                }
            }
            GameState::Settings => {
                if let Some(KeyCode::Escape) = input.keycode {
                    self.state = GameState::Menu;
                }
            }
            GameState::GameOver(_) => {} // Ignore input during game over state
        }
        Ok(())
    }

    fn resize_event(&mut self, ctx: &mut Context, _width: f32, _height: f32) -> GameResult {
        self.handle_window_resize(ctx);
        Ok(())
    }
}

fn main() -> GameResult {
    let (mut ctx, event_loop) = ContextBuilder::new("stack_game", "cursor")
        .window_setup(conf::WindowSetup::default().title("Stack Game"))
        .window_mode(
            conf::WindowMode::default()
                .dimensions(800.0, 600.0)
                .resizable(true)
                .fullscreen_type(conf::FullscreenType::Desktop)
        )
        .build()?;

    let game = GameData::new(&ctx);
    event::run(ctx, event_loop, game)
}

