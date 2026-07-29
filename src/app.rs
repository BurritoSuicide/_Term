use std::collections::VecDeque;

use crate::fx::FxBus;
use crate::procgen::{parse_seed, random_seed_string};
use crate::world::{GamePhase, Vec2, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Title,
    Game,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Explore,
    Inventory,
}

/// Independent held flags so diagonals work (W+D / Up+Right) and dash doesn't wipe other axes.
#[derive(Debug, Clone, Copy, Default)]
pub struct DirKeys {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

impl DirKeys {
    pub fn press_move(&mut self, code: crossterm::event::KeyCode) {
        match code {
            crossterm::event::KeyCode::Char('w') => self.up = true,
            crossterm::event::KeyCode::Char('s') => self.down = true,
            crossterm::event::KeyCode::Char('a') => self.left = true,
            crossterm::event::KeyCode::Char('d') => self.right = true,
            _ => {}
        }
    }

    pub fn release_move(&mut self, code: crossterm::event::KeyCode) {
        match code {
            crossterm::event::KeyCode::Char('w') => self.up = false,
            crossterm::event::KeyCode::Char('s') => self.down = false,
            crossterm::event::KeyCode::Char('a') => self.left = false,
            crossterm::event::KeyCode::Char('d') => self.right = false,
            _ => {}
        }
    }

    pub fn press_aim(&mut self, code: crossterm::event::KeyCode) {
        match code {
            crossterm::event::KeyCode::Up => self.up = true,
            crossterm::event::KeyCode::Down => self.down = true,
            crossterm::event::KeyCode::Left => self.left = true,
            crossterm::event::KeyCode::Right => self.right = true,
            _ => {}
        }
    }

    pub fn release_aim(&mut self, code: crossterm::event::KeyCode) {
        match code {
            crossterm::event::KeyCode::Up => self.up = false,
            crossterm::event::KeyCode::Down => self.down = false,
            crossterm::event::KeyCode::Left => self.left = false,
            crossterm::event::KeyCode::Right => self.right = false,
            _ => {}
        }
    }

    pub fn dir(self) -> Vec2 {
        let mut x = 0.0;
        let mut y = 0.0;
        if self.left {
            x -= 1.0;
        }
        if self.right {
            x += 1.0;
        }
        if self.up {
            y -= 1.0;
        }
        if self.down {
            y += 1.0;
        }
        let v = Vec2::new(x, y);
        if v.length() > 0.0 {
            v.normalized()
        } else {
            Vec2::ZERO
        }
    }
}

pub struct App {
    pub screen: Screen,
    pub ui_mode: UiMode,
    pub seed_input: String,
    pub world: Option<World>,
    pub fx: FxBus,
    pub should_quit: bool,
    pub move_keys: DirKeys,
    pub aim_keys: DirKeys,
    pub options_open: bool,
    /// UI clock for border animation (ticks even while paused).
    pub ui_t: f32,
    /// Rolling FPS samples for the HUD sparkline.
    pub fps_history: VecDeque<f32>,
    last_draw: Option<std::time::Instant>,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Title,
            ui_mode: UiMode::Explore,
            seed_input: random_seed_string(),
            world: None,
            fx: FxBus::new(),
            should_quit: false,
            move_keys: DirKeys::default(),
            aim_keys: DirKeys::default(),
            options_open: false,
            ui_t: 0.0,
            fps_history: VecDeque::with_capacity(64),
            last_draw: None,
        }
    }

    pub fn note_frame(&mut self) {
        let now = std::time::Instant::now();
        if let Some(prev) = self.last_draw {
            let dt = now.duration_since(prev).as_secs_f32().max(1e-4);
            let fps = (1.0 / dt).clamp(1.0, 240.0);
            self.fps_history.push_back(fps);
            while self.fps_history.len() > 48 {
                self.fps_history.pop_front();
            }
        }
        self.last_draw = Some(now);
    }

    pub fn toggle_options(&mut self) {
        self.options_open = !self.options_open;
        if self.options_open {
            self.move_keys = DirKeys::default();
            self.aim_keys = DirKeys::default();
        }
    }

    pub fn options_confirm(&mut self) {
        match self.screen {
            Screen::Title => self.should_quit = true,
            Screen::Game => self.to_title(),
        }
    }

    pub fn start_run(&mut self) {
        let seed = parse_seed(&self.seed_input);
        let label = self.seed_input.trim().to_string();
        self.world = Some(World::new(seed, label));
        self.screen = Screen::Game;
        self.ui_mode = UiMode::Explore;
        self.options_open = false;
        self.move_keys = DirKeys::default();
        self.aim_keys = DirKeys::default();
        self.fx.on_room_enter();
    }

    pub fn to_title(&mut self) {
        self.world = None;
        self.screen = Screen::Title;
        self.ui_mode = UiMode::Explore;
        self.options_open = false;
        self.move_keys = DirKeys::default();
        self.aim_keys = DirKeys::default();
    }

    pub fn toggle_inventory(&mut self) {
        if self.screen != Screen::Game || self.options_open {
            return;
        }
        let Some(world) = self.world.as_mut() else {
            return;
        };
        if world.phase == GamePhase::Dead {
            return;
        }
        // Shop inventory only while the target dummy is active.
        if world.phase == GamePhase::Shop && !world.shop_dummy_active {
            return;
        }
        self.ui_mode = match self.ui_mode {
            UiMode::Explore => UiMode::Inventory,
            UiMode::Inventory => {
                while world.inv_overlay != crate::world::InvOverlay::None {
                    world.inv_close_stash();
                }
                UiMode::Explore
            }
        };
    }

    pub fn move_dir(&self) -> Vec2 {
        self.move_keys.dir()
    }

    pub fn aim_dir(&self) -> Vec2 {
        self.aim_keys.dir()
    }

    pub fn tick(&mut self, dt: f32) {
        self.ui_t += dt;
        let dir = self.move_dir();
        let aim = self.aim_dir();
        let paused = self.ui_mode == UiMode::Inventory || self.options_open;
        if let Some(world) = self.world.as_mut() {
            if !paused && world.phase == GamePhase::Playing {
                world.move_player(dir, dt);
                if aim.length() > 0.0 {
                    world.aim_player(aim);
                }
            } else if !paused && world.phase == GamePhase::Shop {
                world.move_player(dir, dt);
                if world.shop_dummy_active && aim.length() > 0.0 {
                    world.aim_player(aim);
                }
            }
            let before_phase = world.phase;
            let before_room = world.room.combat_index;
            let before_kind = world.room.kind;
            world.update(dt, paused);
            if world.phase == GamePhase::Dead && before_phase != GamePhase::Dead {
                self.fx.on_death();
            }
            if world.room.combat_index != before_room || world.room.kind != before_kind {
                if world.phase == GamePhase::Shop {
                    self.fx.on_shop();
                } else if matches!(world.room.kind, crate::world::RoomKind::Boss) {
                    self.fx.on_boss();
                } else {
                    self.fx.on_room_enter();
                }
            }
        }
        self.fx.tick(dt);
    }
}
