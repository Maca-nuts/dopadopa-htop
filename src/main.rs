use std::{
    cmp::Reverse,
    io::{self, Stdout},
    process::Command,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Sparkline, Table, TableState},
    Frame, Terminal,
};
use sysinfo::{Components, Pid, ProcessesToUpdate, System};

const REFRESH_RATE: Duration = Duration::from_millis(1000);

#[derive(Clone, Debug, Default, PartialEq)]
struct GpuStats {
    name: String,
    core_percent: f64,
    temperature: Option<f64>,
    memory_used: u64,
    memory_total: u64,
}

impl GpuStats {
    fn memory_percent(&self) -> f64 {
        if self.memory_total == 0 {
            0.0
        } else {
            self.memory_used as f64 / self.memory_total as f64 * 100.0
        }
    }
}

#[derive(Clone, Debug)]
struct ProcessRow {
    pid: Pid,
    name: String,
    cpu: f32,
    memory: u64,
}

struct App {
    system: System,
    components: Components,
    processes: Vec<ProcessRow>,
    gpus: Vec<GpuStats>,
    cpu_history: Vec<u64>,
    memory_history: Vec<u64>,
    selected: usize,
    table_state: TableState,
    last_refresh: Instant,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        let mut app = Self {
            system,
            components: Components::new_with_refreshed_list(),
            processes: Vec::new(),
            gpus: Vec::new(),
            cpu_history: Vec::new(),
            memory_history: Vec::new(),
            selected: 0,
            table_state: TableState::default(),
            last_refresh: Instant::now() - REFRESH_RATE,
            should_quit: false,
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        self.components.refresh(true);
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        let cpu = self.system.global_cpu_usage().clamp(0.0, 100.0) as u64;
        let memory = if self.system.total_memory() == 0 {
            0
        } else {
            (self.system.used_memory() * 100 / self.system.total_memory()).min(100)
        };
        push_history(&mut self.cpu_history, cpu);
        push_history(&mut self.memory_history, memory);
        self.processes = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| ProcessRow {
                pid: *pid,
                name: process.name().to_string_lossy().into_owned(),
                cpu: process.cpu_usage(),
                memory: process.memory(),
            })
            .collect();
        self.processes.sort_by_key(|row| Reverse(row.cpu as u64));
        self.selected = self.selected.min(self.processes.len().saturating_sub(1));
        self.table_state
            .select((!self.processes.is_empty()).then_some(self.selected));
        self.gpus = read_gpus();
        self.last_refresh = Instant::now();
    }

    fn on_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.processes.is_empty() {
                    self.selected = (self.selected + 1).min(self.processes.len() - 1);
                    self.table_state.select(Some(self.selected));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                self.table_state
                    .select((!self.processes.is_empty()).then_some(self.selected));
            }
            KeyCode::Char('r') => self.refresh(),
            _ => {}
        }
    }
}

fn push_history(history: &mut Vec<u64>, value: u64) {
    history.push(value);
    if history.len() > 60 {
        history.remove(0);
    }
}

fn read_gpus() -> Vec<GpuStats> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(',').map(str::trim);
            Some(GpuStats {
                name: fields.next()?.to_string(),
                core_percent: fields.next()?.parse().ok()?,
                memory_used: fields.next()?.parse().ok()?,
                memory_total: fields.next()?.parse().ok()?,
                temperature: fields.next().and_then(|value| value.parse().ok()),
            })
        })
        .collect()
}

fn rainbow_gauge<'a>(label: &'a str, ratio: f64, title: &'a str, color: Color) -> Gauge<'a> {
    Gauge::default()
        .block(
            Block::default()
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_set(symbols::border::ROUNDED)
                .border_style(Style::default().fg(Color::Black)),
        )
        .gauge_style(
            Style::default()
                .fg(color)
                .bg(Color::Rgb(20, 22, 35))
                .add_modifier(Modifier::BOLD),
        )
        .label(label.to_string())
        .ratio(ratio.clamp(0.0, 1.0))
}

fn draw(frame: &mut Frame, app: &mut App) {
    // Saturated rainbow colors keep the background lively behind dark text.
    let rainbow = [
        (255u8, 75u8, 115u8),
        (255, 145, 35),
        (255, 225, 35),
        (100, 225, 75),
        (35, 205, 235),
        (155, 135, 255),
        (255, 95, 200),
    ];
    let area = frame.area();
    for x in 0..area.width {
        let position =
            x as f64 * (rainbow.len() - 1) as f64 / area.width.saturating_sub(1).max(1) as f64;
        let index = (position as usize).min(rainbow.len() - 2);
        let blend = position - index as f64;
        let (r1, g1, b1) = rainbow[index];
        let (r2, g2, b2) = rainbow[index + 1];
        let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * blend) as u8;
        let color = Color::Rgb(mix(r1, r2), mix(g1, g2), mix(b1, b2));
        for y in area.y..area.bottom() {
            frame.buffer_mut()[(area.x + x, y)]
                .set_bg(color)
                .set_fg(Color::Black);
        }
    }
    let outer = Block::default()
        .title(Line::from(vec![
            Span::styled("", Style::default().fg(Color::Black)),
            Span::styled(
                "DOPADOPA",
                Style::default()
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" • system monitor ", Style::default().fg(Color::Black)),
            Span::styled("", Style::default().fg(Color::Black)),
        ]))
        .borders(Borders::ALL)
        .border_set(symbols::border::DOUBLE)
        .border_style(Style::default().fg(Color::Black));
    let area = frame.area();
    frame.render_widget(outer, area);
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Min(5),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .split(inner);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[0]);
    let cpu = app.system.global_cpu_usage() as f64;
    let mem = if app.system.total_memory() == 0 {
        0.0
    } else {
        app.system.used_memory() as f64 / app.system.total_memory() as f64 * 100.0
    };
    let temperature = app
        .components
        .iter()
        .filter(|component| {
            let label = component.label().to_ascii_lowercase();
            label.contains("cpu")
                || label.contains("package id")
                || label == "tctl"
                || label == "tdie"
                || label.starts_with("core ")
        })
        .filter_map(|component| component.temperature())
        .filter(|value| value.is_finite())
        .reduce(f32::max);
    let cpu_label = match temperature {
        Some(temp) => format!("{cpu:>5.1}%  {temp:.1}°C"),
        None => format!("{cpu:>5.1}%  temp N/A"),
    };
    frame.render_widget(
        rainbow_gauge(&cpu_label, cpu / 100.0, " CPU" , Color::Rgb(90, 225, 255)),
        top[0],
    );
    frame.render_widget(
        rainbow_gauge(&format!("{mem:>5.1}%"), mem / 100.0, " MEMORY ", Color::Rgb(255, 230, 80)),
        top[1],
    );

    let charts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[1]);
    frame.render_widget(
        Sparkline::default()
            .block(
                Block::default()
                    .title(Span::styled(" CPU history ", Style::default().fg(Color::Black).add_modifier(Modifier::BOLD)))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Black)),
            )
            .data(&app.cpu_history)
            .max(100)
            .style(Style::default().fg(Color::Rgb(0, 35, 105))),
        charts[0],
    );
    frame.render_widget(
        Sparkline::default()
            .block(
                Block::default()
                    .title(Span::styled(" Memory history ", Style::default().fg(Color::Black).add_modifier(Modifier::BOLD)))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Black)),
            )
            .data(&app.memory_history)
            .max(100)
            .style(Style::default().fg(Color::Rgb(0, 70, 35))),
        charts[1],
    );

    let proc_rows = app.processes.iter().map(|p| {
        Row::new(vec![
            Cell::from(p.pid.to_string()),
            Cell::from(p.name.clone()),
            Cell::from(format!("{:>5.1}%", p.cpu)),
            Cell::from(format!("{:>8.1} MB", p.memory as f64 / 1_048_576.0)),
        ])
    });
    let process_block = Block::default()
        .title(" processes ")
        .borders(Borders::ALL)
        .border_set(symbols::border::ROUNDED)
        .border_style(Style::default().fg(Color::Black));
    let table = Table::new(
        proc_rows,
        [
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(14),
        ],
    )
    .header(
        Row::new(vec!["PID", "COMMAND", "CPU", "MEMORY"]).style(
            Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .row_highlight_style(Style::default().bg(Color::Rgb(62, 35, 78)).fg(Color::White))
    .block(process_block);
    frame.render_stateful_widget(table, layout[2], &mut app.table_state);

    let gpu_text = if app.gpus.is_empty() {
        vec![Line::from(Span::styled(
            " nvidia-smi unavailable — GPU metrics hidden",
            Style::default().fg(Color::Black),
        ))]
    } else {
        app.gpus
            .iter()
            .enumerate()
            .flat_map(|(index, gpu)| {
                let color = Color::Black;
                [Line::from(vec![
                    Span::styled(
                        format!(" GPU{} {:<24}", index, gpu.name),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(
                            " CORE {:>5.1}%  TEMP {}  VRAM {:>6} / {:<6} MiB ({:>5.1}%)",
                            gpu.core_percent,
                            gpu.temperature
                                .map(|temp| format!("{temp:.0}°C"))
                                .unwrap_or_else(|| "N/A".to_string()),
                            gpu.memory_used,
                            gpu.memory_total,
                            gpu.memory_percent()
                        ),
                        Style::default().fg(Color::Black),
                    ),
                ])]
            })
            .collect()
    };
    let gpu = Paragraph::new(gpu_text).block(
        Block::default()
            .title(" NVIDIA GPU telemetry ")
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(Color::Black)),
    );
    frame.render_widget(gpu, layout[3]);
    frame.render_widget(
        Paragraph::new(" q quit  r refresh  ↑↓/jk select ")
            .style(Style::default().fg(Color::Black)),
        layout[4],
    );
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    let mut app = App::new();
    while !app.should_quit {
        terminal.draw(|frame| draw(frame, &mut app))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key.code);
            }
        }
        if app.last_refresh.elapsed() >= REFRESH_RATE {
            app.refresh();
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run(&mut terminal);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_capped() {
        let mut history = Vec::new();
        for value in 0..70 {
            push_history(&mut history, value);
        }
        assert_eq!(history.len(), 60);
        assert_eq!(history[0], 10);
        assert_eq!(history[59], 69);
    }

    #[test]
    fn gpu_memory_percentage_handles_empty_total() {
        assert_eq!(GpuStats::default().memory_percent(), 0.0);
        let gpu = GpuStats {
            memory_used: 512,
            memory_total: 2048,
            ..GpuStats::default()
        };
        assert_eq!(gpu.memory_percent(), 25.0);
    }
}
