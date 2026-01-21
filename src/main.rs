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
mod gpu;
mod tegrastats;
mod ui;

use crate::gpu::GpuUtilRunner;
use crate::model::AppState;
use crate::tegrastats::TegrastatsRunner;

#[derive(Parser, Debug)]
#[command(name = "jmon", about = "Jetson monitor TUI using tegrastats")]
struct Args {
    #[arg(short, long, default_value = "tegrastats")]
    tegrastats: String,
    #[arg(long, default_value = "nvidia-smi")]
    nvidia_smi: String,
    #[arg(short, long, default_value_t = 1000)]
    interval: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut runner = TegrastatsRunner::spawn(&args.tegrastats, args.interval).with_context(
        || "failed to start tegrastats (ensure it is installed and accessible without sudo)",
    )?;
    let mut gpu_runner = GpuUtilRunner::spawn(&args.nvidia_smi, args.interval).ok();
    let mut terminal = setup_terminal()?;

    let result = run_app(
        &mut terminal,
        &mut runner,
        &mut gpu_runner,
        &args.tegrastats,
        &args.nvidia_smi,
        args.interval,
    );

    restore_terminal(&mut terminal)?;
    runner.shutdown();
    if let Some(gpu_runner) = gpu_runner.as_mut() {
        gpu_runner.shutdown();
    }

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
    gpu_runner: &mut Option<GpuUtilRunner>,
    tegrastats_path: &str,
    nvidia_smi_path: &str,
    interval_ms: u64,
) -> Result<()> {
    let mut app = AppState::new(interval_ms, 120);
    let mut last_gpu_util: Option<f32> = None;
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    loop {
        let mut latest = None;
        while let Some(snapshot) = runner.try_recv() {
            latest = Some(snapshot);
        }
        if let Some(mut snapshot) = latest {
            snapshot.gpu_util = last_gpu_util;
            app.history.push(&snapshot);
            app.latest = Some(snapshot);
        }

        if let Some(runner) = gpu_runner.as_ref() {
            while let Some(util) = runner.try_recv() {
                last_gpu_util = Some(util);
                if let Some(snapshot) = app.latest.as_mut() {
                    snapshot.gpu_util = Some(util);
                }
            }
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
                        KeyCode::Char('1') => toggle_pane(&mut app, PaneToggle::Cpu),
                        KeyCode::Char('2') => toggle_pane(&mut app, PaneToggle::Ram),
                        KeyCode::Char('3') => toggle_pane(&mut app, PaneToggle::Gpu),
                        KeyCode::Char('4') => toggle_pane(&mut app, PaneToggle::Temps),
                        KeyCode::Char('5') => toggle_pane(&mut app, PaneToggle::Power),
                        KeyCode::Char('h') => app.show_help = !app.show_help,
                        KeyCode::Char('r') => app.history.reset(),
                        KeyCode::Char('+') => {
                            update_interval(
                                runner,
                                gpu_runner,
                                tegrastats_path,
                                nvidia_smi_path,
                                250,
                                &mut app,
                            );
                        }
                        KeyCode::Char('-') => {
                            update_interval(
                                runner,
                                gpu_runner,
                                tegrastats_path,
                                nvidia_smi_path,
                                -250,
                                &mut app,
                            );
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
                                update_interval(
                                    runner,
                                    gpu_runner,
                                    tegrastats_path,
                                    nvidia_smi_path,
                                    -250,
                                    &mut app,
                                );
                                continue;
                            }
                        }
                        if let Some(button) = app.buttons.plus {
                            if button.contains(column, row) {
                                update_interval(
                                    runner,
                                    gpu_runner,
                                    tegrastats_path,
                                    nvidia_smi_path,
                                    250,
                                    &mut app,
                                );
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

fn restart_sources(
    runner: &mut TegrastatsRunner,
    gpu_runner: &mut Option<GpuUtilRunner>,
    path: &str,
    nvidia_smi_path: &str,
    next_interval: u64,
    app: &mut AppState,
) -> Result<()> {
    if next_interval == app.interval_ms {
        return Ok(());
    }
    let new_runner = TegrastatsRunner::spawn(path, next_interval)?;
    runner.shutdown();
    *runner = new_runner;
    if let Some(runner) = gpu_runner.as_mut() {
        runner.shutdown();
    }
    *gpu_runner = GpuUtilRunner::spawn(nvidia_smi_path, next_interval).ok();
    app.interval_ms = next_interval;
    app.error = None;
    Ok(())
}

fn update_interval(
    runner: &mut TegrastatsRunner,
    gpu_runner: &mut Option<GpuUtilRunner>,
    path: &str,
    nvidia_smi_path: &str,
    delta: i64,
    app: &mut AppState,
) {
    let next = if delta.is_negative() {
        let amount = delta.abs() as u64;
        app.interval_ms.saturating_sub(amount).max(250)
    } else {
        (app.interval_ms + delta as u64).min(5000)
    };

    if let Err(err) = restart_sources(runner, gpu_runner, path, nvidia_smi_path, next, app) {
        app.error = Some(err.to_string());
    }
}

enum PaneToggle {
    Cpu,
    Ram,
    Gpu,
    Temps,
    Power,
}

fn toggle_pane(app: &mut AppState, pane: PaneToggle) {
    match pane {
        PaneToggle::Cpu => app.panes.cpu = !app.panes.cpu,
        PaneToggle::Ram => app.panes.ram = !app.panes.ram,
        PaneToggle::Gpu => app.panes.gpu = !app.panes.gpu,
        PaneToggle::Temps => app.panes.temps = !app.panes.temps,
        PaneToggle::Power => app.panes.power = !app.panes.power,
    }
}
