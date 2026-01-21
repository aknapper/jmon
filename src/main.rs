use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

mod model;
mod tegrastats;
mod ui;

use crate::model::AppState;
use crate::tegrastats::TegrastatsRunner;

#[derive(Parser, Debug)]
#[command(name = "jmon", about = "Jetson monitor TUI using tegrastats")]
struct Args {
    #[arg(short, long, default_value = "tegrastats")]
    tegrastats: String,
    #[arg(short, long, default_value_t = 1000)]
    interval: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut runner = TegrastatsRunner::spawn(&args.tegrastats, args.interval).with_context(
        || "failed to start tegrastats (ensure it is installed and accessible without sudo)",
    )?;
    let mut terminal = setup_terminal()?;

    let result = run_app(&mut terminal, &mut runner, &args.tegrastats, args.interval);

    restore_terminal(&mut terminal)?;
    runner.shutdown();

    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    terminal::disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    runner: &mut TegrastatsRunner,
    tegrastats_path: &str,
    interval_ms: u64,
) -> Result<()> {
    let mut app = AppState::new(interval_ms, 120);
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    loop {
        let mut latest = None;
        while let Some(snapshot) = runner.try_recv() {
            latest = Some(snapshot);
        }
        if let Some(snapshot) = latest {
            app.history.push(&snapshot);
            app.latest = Some(snapshot);
        }

        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break
                        }
                        KeyCode::Char('h') => app.show_help = !app.show_help,
                        KeyCode::Char('r') => app.history.reset(),
                        KeyCode::Char('+') => {
                            update_interval(runner, tegrastats_path, 250, &mut app);
                        }
                        KeyCode::Char('-') => {
                            update_interval(runner, tegrastats_path, -250, &mut app);
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => {
                    if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                        let column = mouse.column;
                        let row = mouse.row;
                        if let Some(button) = app.buttons.minus {
                            if button.contains(column, row) {
                                update_interval(runner, tegrastats_path, -250, &mut app);
                                continue;
                            }
                        }
                        if let Some(button) = app.buttons.plus {
                            if button.contains(column, row) {
                                update_interval(runner, tegrastats_path, 250, &mut app);
                            }
                        }
                    } else if mouse.kind == MouseEventKind::Moved {
                        let column = mouse.column;
                        let row = mouse.row;
                        if let Some(button) = app.buttons.minus {
                            if button.contains(column, row) {
                                app.hover = crate::model::HoverTarget::Minus;
                                continue;
                            }
                        }
                        if let Some(button) = app.buttons.plus {
                            if button.contains(column, row) {
                                app.hover = crate::model::HoverTarget::Plus;
                                continue;
                            }
                        }
                        app.hover = crate::model::HoverTarget::None;
                    }
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}

fn restart_tegrastats(
    runner: &mut TegrastatsRunner,
    path: &str,
    next_interval: u64,
    app: &mut AppState,
) -> Result<()> {
    if next_interval == app.interval_ms {
        return Ok(());
    }
    let new_runner = TegrastatsRunner::spawn(path, next_interval)?;
    runner.shutdown();
    *runner = new_runner;
    app.interval_ms = next_interval;
    app.error = None;
    Ok(())
}

fn update_interval(
    runner: &mut TegrastatsRunner,
    path: &str,
    delta: i64,
    app: &mut AppState,
) {
    let next = if delta.is_negative() {
        let amount = delta.abs() as u64;
        app.interval_ms.saturating_sub(amount).max(250)
    } else {
        (app.interval_ms + delta as u64).min(5000)
    };

    if let Err(err) = restart_tegrastats(runner, path, next, app) {
        app.error = Some(err.to_string());
    }
}
