mod app;
mod fx;
mod input;
mod necro;
mod procgen;
mod skills;
mod proj_logic;
mod theme;
mod ui;
mod world;

use std::time::{Duration, Instant};

use color_eyre::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::App;

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = setup_terminal()?;
    let mut app = App::new();

    let tick_rate = Duration::from_secs_f32(1.0 / 60.0);
    let mut last = Instant::now();
    let mut acc = Duration::ZERO;

    let result = run_loop(&mut terminal, &mut app, tick_rate, &mut last, &mut acc);

    restore_terminal(&mut terminal)?;
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    tick_rate: Duration,
    last: &mut Instant,
    acc: &mut Duration,
) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| {
            app.note_frame();
            let focus = ui::draw(frame, app);
            app.fx.render(frame);
            if let Some(rect) = focus {
                theme::paint_animated_border(frame.buffer_mut(), rect, app.ui_t);
            }
        })?;

        let timeout = tick_rate.saturating_sub(last.elapsed());
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => match key.kind {
                    KeyEventKind::Press | KeyEventKind::Repeat => input::handle_key(app, key),
                    KeyEventKind::Release => input::handle_key_release(app, key),
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        let now = Instant::now();
        *acc += now - *last;
        *last = now;
        while *acc >= tick_rate {
            app.tick(tick_rate.as_secs_f32());
            *acc -= tick_rate;
        }
    }
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let _ = execute!(
        stdout,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        )
    );
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
