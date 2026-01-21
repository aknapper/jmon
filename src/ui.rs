use std::collections::VecDeque;

use chrono::Local;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::model::{AppState, HoverTarget, StatsSnapshot, UiButton, UiButtons};

pub fn draw(frame: &mut Frame, app: &mut AppState) {
    let size = frame.size();
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(size);

    render_header(frame, sections[0], app);
    render_body(frame, sections[1], app);

    if app.show_help {
        render_help(frame, size);
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &mut AppState) {
    app.buttons = UiButtons::default();

    let left_line = Line::from(vec![
        Span::styled("jmon", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  q:quit  h:help  r:reset"),
    ]);

    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(30),
            Constraint::Percentage(35),
        ])
        .split(area);

    let header = Paragraph::new(left_line).alignment(Alignment::Left);
    frame.render_widget(header, sections[0]);

    let time_string = Local::now().format("%I:%M:%S %p").to_string();
    let time_line = Paragraph::new(Line::from(time_string)).alignment(Alignment::Center);
    frame.render_widget(time_line, sections[1]);

    render_interval_controls(frame, sections[2], app);
}

fn render_interval_controls(frame: &mut Frame, area: Rect, app: &mut AppState) {
    let label = "interval";
    let minus = "[-]";
    let plus = "[+]";
    let interval_text = format!("{}ms", app.interval_ms);
    let control_len = (label.len() + 1 + minus.len() + 1 + interval_text.len() + 1 + plus.len())
        as u16;

    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(control_len)])
        .split(area);

    let minus_style = if app.hover == HoverTarget::Minus {
        Style::default()
            .fg(Color::LightRed)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };
    let plus_style = if app.hover == HoverTarget::Plus {
        Style::default()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    };

    let line = Line::from(vec![
        Span::raw(format!("{} ", label)),
        Span::styled(minus, minus_style),
        Span::raw(format!(" {} ", interval_text)),
        Span::styled(plus, plus_style),
    ]);
    let paragraph = Paragraph::new(line).alignment(Alignment::Left);
    frame.render_widget(paragraph, sections[1]);

    let minus_start = label.len() + 1;
    let plus_start = label.len() + 1 + minus.len() + 1 + interval_text.len() + 1;
    let width = sections[1].width as usize;

    if width >= minus_start + minus.len() {
        app.buttons.minus = Some(UiButton {
            x: sections[1].x + minus_start as u16,
            y: sections[1].y,
            width: minus.len() as u16,
        });
    }

    if width >= plus_start + plus.len() {
        app.buttons.plus = Some(UiButton {
            x: sections[1].x + plus_start as u16,
            y: sections[1].y,
            width: plus.len() as u16,
        });
    }

    if let Some(error) = &app.error {
        let error_line = Paragraph::new(Line::from(Span::styled(
            format!("error: {}", error),
            Style::default().fg(Color::Red),
        )))
        .alignment(Alignment::Right);
        frame.render_widget(error_line, sections[0]);
    }
}

fn render_body(frame: &mut Frame, area: Rect, app: &AppState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(columns[0]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(25),
            Constraint::Percentage(40),
        ])
        .split(columns[1]);

    render_cpu_panel(frame, left[0], app);
    render_ram_panel(frame, left[1], app);
    render_gpu_panel(frame, right[0], app);
    render_temps_panel(frame, right[1], app);
    render_power_panel(frame, right[2], app);
}

fn render_cpu_panel(frame: &mut Frame, area: Rect, app: &AppState) {
    let title = match app.latest.as_ref().and_then(StatsSnapshot::cpu_total) {
        Some(total) => format!("CPU {:.0}%", total),
        None => "CPU".to_string(),
    };

    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(inner);

    let core_lines = match app.latest.as_ref() {
        Some(snapshot) if !snapshot.cpu_cores.is_empty() => snapshot
            .cpu_cores
            .iter()
            .enumerate()
            .map(|(idx, util)| core_bar_line(idx, *util, sections[0].width, SparkRgb::cpu()))
            .collect(),
        Some(_) => vec![Line::from("No CPU data")],
        None => vec![Line::from("Waiting for tegrastats...")],
    };

    let core_list = Paragraph::new(core_lines).alignment(Alignment::Left);
    frame.render_widget(core_list, sections[0]);

    let cpu_spark = sparkline_data(&app.history.cpu_total, sections[1].width);
    render_sparkline(frame, sections[1], &cpu_spark, SparkRgb::cpu(), Some(100));
}

fn render_ram_panel(frame: &mut Frame, area: Rect, app: &AppState) {
    let title = match app
        .latest
        .as_ref()
        .and_then(StatsSnapshot::ram_percent)
    {
        Some(percent) => format!("RAM {:.0}%", percent),
        None => "RAM".to_string(),
    };

    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(3)])
        .split(inner);

    let line = match app.latest.as_ref() {
        Some(snapshot) => memory_bar_line(snapshot, sections[0].width, SparkRgb::ram()),
        None => Line::from("Waiting for tegrastats..."),
    };
    frame.render_widget(Paragraph::new(line), sections[0]);

    let ram_spark = sparkline_data(&app.history.ram_used, sections[1].width);
    let ram_max = app.latest.as_ref().and_then(|snapshot| snapshot.ram_total_mb);
    render_sparkline(frame, sections[1], &ram_spark, SparkRgb::ram(), ram_max);
}

fn render_gpu_panel(frame: &mut Frame, area: Rect, app: &AppState) {
    let title = match app.latest.as_ref().and_then(|snap| snap.gpu_util) {
        Some(util) => format!("GPU {:.0}%", util),
        None => "GPU".to_string(),
    };

    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(3)])
        .split(inner);

    let mut lines = Vec::new();
    if let Some(snapshot) = app.latest.as_ref() {
        if let Some(util) = snapshot.gpu_util {
            lines.push(bar_line("GPU", util, sections[0].width, SparkRgb::gpu()));
        } else {
            lines.push(Line::from("GPU: N/A"));
        }

        if let Some(emc) = snapshot.emc_util {
            lines.push(bar_line("EMC", emc, sections[0].width, SparkRgb::emc()));
        }
    } else {
        lines.push(Line::from("Waiting for tegrastats..."));
    }

    frame.render_widget(Paragraph::new(lines), sections[0]);

    let gpu_spark = sparkline_data(&app.history.gpu_util, sections[1].width);
    render_sparkline(frame, sections[1], &gpu_spark, SparkRgb::gpu(), Some(100));
}

fn render_power_panel(frame: &mut Frame, area: Rect, app: &AppState) {
    let title = match app.latest.as_ref().and_then(StatsSnapshot::total_power_mw) {
        Some(total) => format!("Power {}mW", total),
        None => "Power".to_string(),
    };

    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(3), Constraint::Length(3)])
        .split(inner);

    let total_line = match app.latest.as_ref().and_then(StatsSnapshot::total_power_mw) {
        Some(total) => {
            let max_power = app
                .history
                .power_total
                .iter()
                .copied()
                .max()
                .unwrap_or(total)
                .max(1);
            let percent = (total as f64 / max_power as f64) * 100.0;
            power_bar_line(total, percent, sections[0].width, SparkRgb::power())
        }
        None => Line::from("Waiting for tegrastats..."),
    };
    frame.render_widget(Paragraph::new(total_line), sections[0]);

    let rail_lines = match app.latest.as_ref() {
        Some(snapshot) if !snapshot.power_rails.is_empty() => snapshot
            .power_rails
            .iter()
            .map(|rail| {
                Line::from(format!(
                    "{:<16} {:>6}mW / {:>6}mW",
                    rail.name, rail.current_mw, rail.average_mw
                ))
            })
            .collect(),
        Some(_) => vec![Line::from("No power rails")],
        None => vec![Line::from("Waiting for tegrastats...")],
    };
    frame.render_widget(Paragraph::new(rail_lines), sections[1]);

    let power_spark = sparkline_data(&app.history.power_total, sections[2].width);
    render_sparkline(frame, sections[2], &power_spark, SparkRgb::power(), None);
}

fn render_temps_panel(frame: &mut Frame, area: Rect, app: &AppState) {
    let block = Block::default().title("Temps").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = match app.latest.as_ref() {
        Some(snapshot) if !snapshot.temps.is_empty() => snapshot
            .temps
            .iter()
            .map(|temp| temp_line(&temp.name, temp.value_c))
            .collect(),
        Some(_) => vec![Line::from("No temps")],
        None => vec![Line::from("Waiting for tegrastats...")],
    };

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help_area = centered_rect(60, 40, area);
    let block = Block::default().title("Help").borders(Borders::ALL);
    let lines = vec![
        Line::from("q / Esc  quit"),
        Line::from("h        toggle help"),
        Line::from("r        reset history"),
        Line::from("+/-      change tegrastats interval"),
    ];
    let paragraph = Paragraph::new(lines).alignment(Alignment::Left).block(block);
    frame.render_widget(Clear, help_area);
    frame.render_widget(paragraph, help_area);
}

fn core_bar_line(index: usize, percent: f32, width: u16, target: SparkRgb) -> Line<'static> {
    let label = format!("C{:02}", index);
    let percent_text = format!("{:>3.0}%", percent);
    let bar_width = width
        .saturating_sub(label.len() as u16 + percent_text.len() as u16 + 4)
        as usize;
    let bar = make_bar(percent as f64, bar_width);
    let color = scaled_color(target, percent as f64);
    let percent_color = heat_color(percent as f64, 0.0, 50.0, 100.0);

    Line::from(vec![
        Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(format!("[{}]", bar), Style::default().fg(color)),
        Span::raw(" "),
        Span::styled(percent_text, Style::default().fg(percent_color)),
    ])
}

fn memory_bar_line(snapshot: &StatsSnapshot, width: u16, target: SparkRgb) -> Line<'static> {
    let (used, total, percent) = match (snapshot.ram_used_mb, snapshot.ram_total_mb) {
        (Some(used), Some(total)) if total > 0 => {
            let percent = (used as f64 / total as f64) * 100.0;
            (used, total, percent)
        }
        _ => return Line::from("RAM data unavailable"),
    };

    let label = "RAM";
    let suffix = format!("{}/{}MB", used, total);
    let bar_width = width
        .saturating_sub(label.len() as u16 + suffix.len() as u16 + 5)
        as usize;
    let bar = make_bar(percent, bar_width);
    let color = scaled_color(target, percent);

    Line::from(vec![
        Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(format!("[{}]", bar), Style::default().fg(color)),
        Span::raw(" "),
        Span::raw(suffix),
    ])
}

fn power_bar_line(total_mw: u64, percent: f64, width: u16, target: SparkRgb) -> Line<'static> {
    let label = "TOTAL";
    let suffix = format!("{}mW", total_mw);
    let bar_width = width
        .saturating_sub(label.len() as u16 + suffix.len() as u16 + 5)
        as usize;
    let bar = make_bar(percent, bar_width);
    let color = scaled_color(target, percent);

    Line::from(vec![
        Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(format!("[{}]", bar), Style::default().fg(color)),
        Span::raw(" "),
        Span::raw(suffix),
    ])
}

fn temp_line(name: &str, value_c: f32) -> Line<'static> {
    let label = name.to_string();
    let value = format!("{:>5.1}C", value_c);
    let label_style = Style::default().fg(Color::Gray);
    let value_color = heat_color(value_c as f64, 30.0, 60.0, 85.0);
    let value_style = Style::default().fg(value_color).add_modifier(Modifier::BOLD);

    Line::from(vec![
        Span::styled(label, label_style),
        Span::raw(" "),
        Span::styled(value, value_style),
    ])
}

fn bar_line(label: &str, percent: f32, width: u16, target: SparkRgb) -> Line<'static> {
    let label = label.to_string();
    let percent_text = format!("{:>3.0}%", percent);
    let bar_width = width
        .saturating_sub(label.len() as u16 + percent_text.len() as u16 + 4)
        as usize;
    let bar = make_bar(percent as f64, bar_width);
    let color = scaled_color(target, percent as f64);
    let percent_color = heat_color(percent as f64, 0.0, 50.0, 100.0);

    Line::from(vec![
        Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(format!("[{}]", bar), Style::default().fg(color)),
        Span::raw(" "),
        Span::styled(percent_text, Style::default().fg(percent_color)),
    ])
}

fn make_bar(percent: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let filled = ((percent / 100.0) * width as f64).round().clamp(0.0, width as f64) as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "#".repeat(filled), "-".repeat(empty))
}

fn sparkline_data(data: &VecDeque<u64>, width: u16) -> Vec<u64> {
    let width = width as usize;
    if width == 0 {
        return Vec::new();
    }
    if data.is_empty() {
        return vec![0; width];
    }

    let mut values = vec![0; width];
    let available = data.len().min(width);
    let start_idx = data.len().saturating_sub(available);
    let target_start = width.saturating_sub(available);

    for (idx, value) in data.iter().skip(start_idx).take(available).enumerate() {
        values[target_start + idx] = *value;
    }

    values
}

fn render_sparkline(
    frame: &mut Frame,
    area: Rect,
    data: &[u64],
    target: SparkRgb,
    max_override: Option<u64>,
) {
    if area.is_empty() || data.is_empty() {
        return;
    }

    let max = max_override.unwrap_or_else(|| data.iter().copied().max().unwrap_or(1).max(1));
    let height = area.height as u64;
    let bar_set = symbols::bar::NINE_LEVELS;
    let base = SparkRgb::base();

    let buffer = frame.buffer_mut();
    let width = area.width as usize;

    for (i, value) in data.iter().take(width).enumerate() {
        let mut scaled = value.saturating_mul(height * 8) / max;
        let intensity = adjust_intensity(*value as f64 / max as f64);
        let color = blend_color(base, target, intensity);

        for row in 0..area.height {
            let symbol = match scaled {
                0 => bar_set.empty,
                1 => bar_set.one_eighth,
                2 => bar_set.one_quarter,
                3 => bar_set.three_eighths,
                4 => bar_set.half,
                5 => bar_set.five_eighths,
                6 => bar_set.three_quarters,
                7 => bar_set.seven_eighths,
                _ => bar_set.full,
            };
            let x = area.left() + i as u16;
            let y = area.bottom().saturating_sub(1 + row);
            buffer
                .get_mut(x, y)
                .set_symbol(symbol)
                .set_style(Style::default().fg(color));

            if scaled > 8 {
                scaled -= 8;
            } else {
                scaled = 0;
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SparkRgb {
    r: u8,
    g: u8,
    b: u8,
}

impl SparkRgb {
    const fn base() -> Self {
        Self { r: 255, g: 255, b: 255 }
    }

    const fn cpu() -> Self {
        Self { r: 40, g: 200, b: 120 }
    }

    const fn ram() -> Self {
        Self { r: 230, g: 180, b: 30 }
    }

    const fn gpu() -> Self {
        Self { r: 70, g: 200, b: 200 }
    }

    const fn emc() -> Self {
        Self { r: 90, g: 140, b: 230 }
    }

    const fn power() -> Self {
        Self { r: 220, g: 90, b: 90 }
    }

    const fn cool() -> Self {
        Self { r: 60, g: 150, b: 255 }
    }

    const fn warm() -> Self {
        Self { r: 255, g: 210, b: 0 }
    }

    const fn hot() -> Self {
        Self { r: 255, g: 90, b: 90 }
    }
}

fn blend_color(base: SparkRgb, target: SparkRgb, t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let r = base.r as f64 + (target.r as f64 - base.r as f64) * t;
    let g = base.g as f64 + (target.g as f64 - base.g as f64) * t;
    let b = base.b as f64 + (target.b as f64 - base.b as f64) * t;

    Color::Rgb(r.round() as u8, g.round() as u8, b.round() as u8)
}

fn blend_rgb(start: SparkRgb, end: SparkRgb, t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let r = start.r as f64 + (end.r as f64 - start.r as f64) * t;
    let g = start.g as f64 + (end.g as f64 - start.g as f64) * t;
    let b = start.b as f64 + (end.b as f64 - start.b as f64) * t;

    Color::Rgb(r.round() as u8, g.round() as u8, b.round() as u8)
}

fn scaled_color(target: SparkRgb, percent: f64) -> Color {
    let t = adjust_intensity(percent / 100.0);
    blend_color(SparkRgb::base(), target, t)
}

fn adjust_intensity(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 {
        0.0
    } else {
        t.powf(0.6)
    }
}

fn heat_color(value: f64, low: f64, mid: f64, high: f64) -> Color {
    let value = value.clamp(low, high);
    if value <= mid {
        let t = if mid <= low { 0.0 } else { (value - low) / (mid - low) };
        blend_rgb(SparkRgb::cool(), SparkRgb::warm(), t)
    } else {
        let t = if high <= mid { 1.0 } else { (value - mid) / (high - mid) };
        blend_rgb(SparkRgb::warm(), SparkRgb::hot(), t)
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1]);

    horizontal[1]
}
