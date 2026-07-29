use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, ModifierKeyCode};

use crate::app::{App, Screen, UiMode};
use crate::world::{GamePhase, Vec2};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Char('q')
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
    {
        app.should_quit = true;
        return;
    }

    // Esc toggles pause (except closing stash first).
    if key.code == KeyCode::Esc {
        if app.screen == Screen::Game
            && app.ui_mode == UiMode::Inventory
            && app
                .world
                .as_ref()
                .is_some_and(|w| w.stash_pick_open() || w.mod_menu_open())
        {
            app.world.as_mut().unwrap().inv_close_stash();
            return;
        }
        app.toggle_options();
        return;
    }

    if app.options_open {
        if key.code == KeyCode::Enter {
            app.options_confirm();
        }
        return;
    }

    match app.screen {
        Screen::Title => handle_title(app, key),
        Screen::Game => handle_game(app, key),
    }
}

fn handle_title(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => app.start_run(),
        KeyCode::Char('d') => {
            app.seed_input = crate::procgen::random_seed_string();
        }
        KeyCode::Backspace => {
            app.seed_input.pop();
        }
        KeyCode::Char(c) if c.is_ascii_alphanumeric() || c == '-' || c == '_' => {
            if app.seed_input.len() < 32 {
                app.seed_input.push(c);
            }
        }
        _ => {}
    }
}

fn handle_game(app: &mut App, key: KeyEvent) {
    if app.world.is_none() {
        return;
    }

    let phase = app.world.as_ref().unwrap().phase;
    let facing = app.world.as_ref().unwrap().player.facing;

    if phase == GamePhase::Dead {
        if key.code == KeyCode::Enter {
            app.to_title();
        }
        return;
    }

    if phase == GamePhase::Shop {
        // Inventory while dummy is active (loadout / DPS testing).
        if key.code == KeyCode::Tab {
            app.toggle_inventory();
            return;
        }
        if app.ui_mode == UiMode::Inventory {
            let world = app.world.as_mut().unwrap();
            match key.code {
                KeyCode::Backspace => {
                    if world.stash_pick_open() || world.mod_menu_open() {
                        world.inv_close_stash();
                    }
                }
                KeyCode::Char('j') | KeyCode::Down => world.inv_move(1),
                KeyCode::Char('k') | KeyCode::Up => world.inv_move(-1),
                KeyCode::Char('h') | KeyCode::Left => world.inv_swap_adjacent(-1),
                KeyCode::Char('l') | KeyCode::Right => world.inv_swap_adjacent(1),
                KeyCode::Char('f') => world.inv_toggle_focus(),
                KeyCode::Enter | KeyCode::Char(' ') => world.inv_confirm(),
                _ => {}
            }
            return;
        }
        let world = app.world.as_mut().unwrap();
        match key.code {
            KeyCode::Char('t') => {
                let was = world.shop_dummy_active;
                world.toggle_shop_dummy();
                if was && !world.shop_dummy_active && app.ui_mode == UiMode::Inventory {
                    app.ui_mode = UiMode::Explore;
                    world.inv_close_stash();
                }
            }
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    world.leave_shop();
                    app.ui_mode = UiMode::Explore;
                    app.fx.on_room_enter();
                } else {
                    world.try_shop_interact();
                }
            }
            KeyCode::Char('c') => {
                world.leave_shop();
                app.ui_mode = UiMode::Explore;
                app.fx.on_room_enter();
            }
            KeyCode::Char('w')
            | KeyCode::Char('a')
            | KeyCode::Char('s')
            | KeyCode::Char('d') => set_move_key(app, key.code, true),
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right
                if world.shop_dummy_active =>
            {
                set_aim_key(app, key.code, true);
            }
            _ => {}
        }
        return;
    }

    if key.code == KeyCode::Tab {
        app.toggle_inventory();
        return;
    }

    if app.ui_mode == UiMode::Inventory {
        let world = app.world.as_mut().unwrap();
        match key.code {
            KeyCode::Backspace => {
                if world.stash_pick_open() || world.mod_menu_open() {
                    world.inv_close_stash();
                }
            }
            KeyCode::Char('j') | KeyCode::Down => world.inv_move(1),
            KeyCode::Char('k') | KeyCode::Up => world.inv_move(-1),
            KeyCode::Char('h') | KeyCode::Left => world.inv_swap_adjacent(-1),
            KeyCode::Char('l') | KeyCode::Right => world.inv_swap_adjacent(1),
            KeyCode::Char('f') => world.inv_toggle_focus(),
            KeyCode::Enter | KeyCode::Char(' ') => world.inv_confirm(),
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Modifier(ModifierKeyCode::LeftShift)
        | KeyCode::Modifier(ModifierKeyCode::RightShift) => {
            let dir = dash_dir_from(app.move_dir(), facing);
            app.world.as_mut().unwrap().try_dash(dir);
        }
        KeyCode::Char(' ') => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                let dir = dash_dir_from(app.move_dir(), facing);
                app.world.as_mut().unwrap().try_dash(dir);
            }
            // Nucleus autofires — Space (unshifted) has no cast action anymore.
        }
        KeyCode::Char('r') => {
            // Manual raise — walking over a corpse also auto-raises, no fx here.
            app.world.as_mut().unwrap().try_resurrect();
        }
        KeyCode::Enter => {
            let _ = app.world.as_mut().unwrap().try_advance_door();
        }
        KeyCode::Char('w')
        | KeyCode::Char('a')
        | KeyCode::Char('s')
        | KeyCode::Char('d') => {
            set_move_key(app, key.code, true);
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                let dir = match key.code {
                    KeyCode::Char('w') => Vec2::new(0.0, -1.0),
                    KeyCode::Char('s') => Vec2::new(0.0, 1.0),
                    KeyCode::Char('a') => Vec2::new(-1.0, 0.0),
                    KeyCode::Char('d') => Vec2::new(1.0, 0.0),
                    _ => dash_dir_from(app.move_dir(), facing),
                };
                app.world.as_mut().unwrap().try_dash(dir);
            }
        }
        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
            set_aim_key(app, key.code, true);
        }
        _ => {}
    }
}

pub fn handle_key_release(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('w')
        | KeyCode::Char('a')
        | KeyCode::Char('s')
        | KeyCode::Char('d') => set_move_key(app, key.code, false),
        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
            set_aim_key(app, key.code, false);
        }
        _ => {}
    }
}

fn dash_dir_from(held: Vec2, facing: Vec2) -> Vec2 {
    if held.length() > 0.05 {
        held
    } else if facing.length() > 0.05 {
        facing
    } else {
        Vec2::new(1.0, 0.0)
    }
}

fn set_move_key(app: &mut App, code: KeyCode, pressed: bool) {
    if pressed {
        app.move_keys.press_move(code);
    } else {
        app.move_keys.release_move(code);
    }
}

fn set_aim_key(app: &mut App, code: KeyCode, pressed: bool) {
    if pressed {
        app.aim_keys.press_aim(code);
    } else {
        app.aim_keys.release_aim(code);
    }
}
