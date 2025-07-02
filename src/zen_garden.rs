use color_eyre::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Gauge, List, ListItem, Paragraph,
    },
    Frame, Terminal,
};
use std::{
    io,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

use crate::{
    cli::CliArgs,
    database::clean_vscode_databases,
    filesystem::find_vscode_storage_directories,
    storage::update_vscode_storage,
};

#[derive(Debug, Clone)]
pub enum ZenEvent {
    StartScanning,
    ProcessFound(ProcessStone),
    LocationFound(String),
    ProcessTerminated(String),
    StorageUpdated(String),
    DatabaseCleaned(String),
    OperationComplete,
    Error(String),
    LogMessage(String),
    SetTotalOperations(usize),
}

#[derive(Debug, Clone)]
pub struct ProcessStone {
    pub name: String,
    pub pid: u32,
    pub path: String,
    pub is_selected: bool,
    pub is_terminated: bool,
}

impl std::fmt::Display for ProcessStone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.pid)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ZenState {
    Welcome,
    Scanning,
    Processing,
    Complete,
    Error,
}

pub struct ZenGarden {
    state: ZenState,
    events: Vec<String>,
    processes: Vec<ProcessStone>,
    locations: Vec<String>,
    progress: f64,
    total_operations: usize,
    completed_operations: usize,
    current_operation: String,
    start_time: Instant,
    breathing_phase: f64,
    water_flow: usize,
    should_quit: bool,
    selected_stone: usize,
}

impl ZenGarden {
    pub fn new() -> Self {
        Self {
            state: ZenState::Welcome,
            events: Vec::new(),
            processes: Vec::new(),
            locations: Vec::new(),
            progress: 0.0,
            total_operations: 0,
            completed_operations: 0,
            current_operation: "preparing meditation space...".to_string(),
            start_time: Instant::now(),
            breathing_phase: 0.0,
            water_flow: 0,
            should_quit: false,
            selected_stone: 0,
        }
    }

    pub async fn run(&mut self, args: CliArgs) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let (tx, mut rx) = mpsc::unbounded_channel();

        // spawn background task for operations
        let tx_clone = tx.clone();
        let args_clone = args.clone();
        tokio::spawn(async move {
            zen_operations(tx_clone, args_clone).await;
        });

        let mut last_tick = Instant::now();
        let tick_rate = Duration::from_millis(16); // ~60fps for smooth progress updates

        loop {
            terminal.draw(|f| self.ui(f))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if crossterm::event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                self.should_quit = true;
                                break;
                            }
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                if self.state == ZenState::Welcome {
                                    self.state = ZenState::Scanning;
                                    let _ = tx.send(ZenEvent::StartScanning);
                                } else if self.state == ZenState::Scanning || self.state == ZenState::Processing {
                                    // terminate selected process
                                    if let Some(stone) = self.processes.get_mut(self.selected_stone) {
                                        if !stone.is_terminated {
                                            let pid = stone.pid;
                                            let name = stone.name.clone();
                                            stone.is_terminated = true;
                                            let _ = tx.send(ZenEvent::ProcessTerminated(name));
                                            self.terminate_process(pid);
                                        }
                                    }
                                }
                            }
                            KeyCode::Up => {
                                if !self.processes.is_empty() && self.selected_stone > 0 {
                                    self.selected_stone -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if !self.processes.is_empty() && self.selected_stone < self.processes.len() - 1 {
                                    self.selected_stone += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            // handle zen events
            while let Ok(event) = rx.try_recv() {
                self.handle_event(event);
            }

            if last_tick.elapsed() >= tick_rate {
                self.update_animations();
                last_tick = Instant::now();
            }

            if self.should_quit {
                break;
            }
        }

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        Ok(())
    }

    fn handle_event(&mut self, event: ZenEvent) {
        match event {
            ZenEvent::StartScanning => {
                self.state = ZenState::Scanning;
                self.current_operation = "scanning digital landscape...".to_string();
            }
            ZenEvent::ProcessFound(process) => {
                self.processes.push(process.clone());
                self.events.push(format!("discovered restless spirit: {}", process));
            }
            ZenEvent::LocationFound(location) => {
                self.locations.push(location.clone());
                self.events.push(format!("found sacred grove: {}", location));
            }
            ZenEvent::ProcessTerminated(process) => {
                self.events.push(format!("gently guided {} to peaceful rest", process));
                self.completed_operations += 1;
                self.update_progress();
            }
            ZenEvent::StorageUpdated(location) => {
                self.events.push(format!("cleansed energy patterns in {}", location));
                self.completed_operations += 1;
                self.update_progress();
            }
            ZenEvent::DatabaseCleaned(location) => {
                self.events.push(format!("purified data streams in {}", location));
                self.completed_operations += 1;
                self.update_progress();
            }
            ZenEvent::OperationComplete => {
                self.state = ZenState::Complete;
                self.current_operation = "digital harmony achieved".to_string();
                self.progress = 1.0;
            }
            ZenEvent::Error(error) => {
                self.state = ZenState::Error;
                self.events.push(format!("encountered turbulence: {}", error));
            }
            ZenEvent::LogMessage(message) => {
                self.events.push(message);
            }
            ZenEvent::SetTotalOperations(total) => {
                self.total_operations = total;
                self.completed_operations = 0;
                self.progress = 0.0;
            }
        }
    }

    fn update_progress(&mut self) {
        if self.total_operations > 0 {
            self.progress = self.completed_operations as f64 / self.total_operations as f64;
        }
    }

    fn update_animations(&mut self) {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        self.breathing_phase = (elapsed * 0.5).sin() * 0.5 + 0.5;
        self.water_flow = (elapsed * 2.0) as usize % 20;
    }

    fn terminate_process(&self, pid: u32) {
        use kill_tree::blocking::kill_tree;
        let _ = kill_tree(pid);
    }

    fn ui(&self, f: &mut Frame) {
        let size = f.area();

        // main container with zen styling
        let main_block = Block::default()
            .title("🌸 privacy zen garden 🌸")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .style(Style::default().bg(Color::Black));

        f.render_widget(main_block, size);

        let inner = size.inner(Margin { horizontal: 2, vertical: 1 });

        match self.state {
            ZenState::Welcome => self.render_welcome(f, inner),
            ZenState::Scanning | ZenState::Processing => self.render_meditation(f, inner),
            ZenState::Complete => self.render_enlightenment(f, inner),
            ZenState::Error => self.render_turbulence(f, inner),
        }
    }

    fn render_welcome(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        // welcome message
        let welcome = Paragraph::new(Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                "welcome to the digital zen garden",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC),
            )),
            Line::from(""),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(welcome, chunks[0]);

        // meditation space
        let meditation_text = vec![
            Line::from(""),
            Line::from("                    🪨 interactive meditation stones"),
            Line::from("                  ○ select processes to close peacefully"),
            Line::from("                  ◉ navigate with ↑↓, close with enter"),
            Line::from(""),
            Line::from("            🌊 flowing water cleanses all attachments 🌊"),
            Line::from(""),
            Line::from("                    🍃 gentle breeze carries"),
            Line::from("                  away digital impurities"),
            Line::from(""),
            Line::from("                    🌱 new growth emerges"),
            Line::from("                  from mindful cleansing"),
            Line::from(""),
        ];

        let meditation = Paragraph::new(meditation_text)
            .style(Style::default().fg(Color::Green))
            .alignment(Alignment::Center);
        f.render_widget(meditation, chunks[1]);

        // instructions
        let instructions = Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                "press [enter] to scan for processes • [↑↓] to select stones • [enter] to close • [q] to exit",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM),
            )),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(instructions, chunks[2]);
    }

    fn render_meditation(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(8),
                Constraint::Length(6),
            ])
            .split(area);

        // current operation
        let operation = Paragraph::new(Line::from(Span::styled(
            &self.current_operation,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC),
        )))
        .alignment(Alignment::Center);
        f.render_widget(operation, chunks[0]);

        // meditation space with dynamic elements
        let middle_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(chunks[1]);

        self.render_meditation_stones(f, middle_chunks[0]);
        self.render_flowing_water(f, middle_chunks[1]);
        self.render_gentle_breeze(f, middle_chunks[2]);

        // zen log
        self.render_zen_log(f, chunks[2]);
    }

    fn render_meditation_stones(&self, f: &mut Frame, area: Rect) {
        let stones_block = Block::default()
            .title("🪨 meditation stones (↑↓ to select, enter to close)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        let stones_area = stones_block.inner(area);
        f.render_widget(stones_block, area);

        let stone_items: Vec<ListItem> = self.processes.iter()
            .enumerate()
            .map(|(i, stone)| {
                let symbol = if stone.is_terminated {
                    "●" // solid stone - terminated
                } else if i == self.selected_stone {
                    "◉" // selected stone
                } else {
                    "○" // empty stone - active
                };

                let style = if stone.is_terminated {
                    Style::default().fg(Color::DarkGray)
                } else if i == self.selected_stone {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(format!("{} {}", symbol, stone.name))
                    .style(style)
            })
            .collect();

        let stones_list = List::new(stone_items)
            .style(Style::default().fg(Color::White));
        f.render_widget(stones_list, stones_area);
    }

    fn render_flowing_water(&self, f: &mut Frame, area: Rect) {
        let water_block = Block::default()
            .title("🌊 flowing water")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));

        let water_area = water_block.inner(area);
        f.render_widget(water_block, area);

        // animated water flow
        let water_chars = vec!['░', '▒', '▓', '█', '▓', '▒', '░', '░', '▒', '▓', '█', '▓', '▒', '░'];
        let flow_start = self.water_flow % water_chars.len();
        let flow_text: String = water_chars.iter()
            .cycle()
            .skip(flow_start)
            .take(20)
            .collect();

        let progress_gauge = Gauge::default()
            .block(Block::default())
            .gauge_style(Style::default().fg(Color::Blue))
            .percent((self.progress * 100.0).min(100.0) as u16)
            .label(format!("cleansing... {:.0}%", (self.progress * 100.0).min(100.0)));

        let water_content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(water_area);

        let flow = Paragraph::new(flow_text)
            .style(Style::default().fg(Color::Blue))
            .alignment(Alignment::Center);
        f.render_widget(flow, water_content[0]);

        f.render_widget(progress_gauge, water_content[1]);
    }

    fn render_gentle_breeze(&self, f: &mut Frame, area: Rect) {
        let breeze_block = Block::default()
            .title("🍃 gentle breeze")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        let breeze_area = breeze_block.inner(area);
        f.render_widget(breeze_block, area);

        let location_items: Vec<ListItem> = self.locations.iter()
            .map(|l| ListItem::new(format!("🌱 {}", l)))
            .collect();

        let locations_list = List::new(location_items)
            .style(Style::default().fg(Color::Green));
        f.render_widget(locations_list, breeze_area);
    }

    fn render_zen_log(&self, f: &mut Frame, area: Rect) {
        let log_block = Block::default()
            .title("🌸 mindful observations")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let log_area = log_block.inner(area);
        f.render_widget(log_block, area);

        let log_items: Vec<ListItem> = self.events.iter()
            .rev()
            .take(4)
            .map(|event| ListItem::new(format!("• {}", event)))
            .collect();

        let log_list = List::new(log_items)
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(log_list, log_area);
    }

    fn render_enlightenment(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        // completion message
        let completion = Paragraph::new(Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                "🌸 digital harmony achieved 🌸",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(completion, chunks[0]);

        // enlightenment garden
        let garden_text = vec![
            Line::from(""),
            Line::from("                    🌸 cherry blossoms bloom"),
            Line::from("                  in the purified digital space"),
            Line::from(""),
            Line::from("            🧘 inner peace flows through clean pathways 🧘"),
            Line::from(""),
            Line::from("                    🌟 privacy illuminated"),
            Line::from("                  telemetry shadows dissolved"),
            Line::from(""),
            Line::from("                    🕊️ digital freedom achieved"),
            Line::from("                  mindful computing restored"),
            Line::from(""),
        ];

        let garden = Paragraph::new(garden_text)
            .style(Style::default().fg(Color::Magenta))
            .alignment(Alignment::Center);
        f.render_widget(garden, chunks[1]);

        // exit instructions
        let exit_text = Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                "press [q] to return to the world with renewed digital mindfulness",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM),
            )),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(exit_text, chunks[2]);
    }

    fn render_turbulence(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        // error message
        let error_msg = Paragraph::new(Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                "🌪️ turbulence in the digital realm 🌪️",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(error_msg, chunks[0]);

        // turbulence description
        let turbulence_text = vec![
            Line::from(""),
            Line::from("                    ⚡ unexpected energy patterns"),
            Line::from("                  have disrupted the meditation"),
            Line::from(""),
            Line::from("            🌊 breathe deeply, center yourself 🌊"),
            Line::from(""),
            Line::from("                    🔄 the garden will restore"),
            Line::from("                  balance when conditions align"),
            Line::from(""),
        ];

        let turbulence = Paragraph::new(turbulence_text)
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        f.render_widget(turbulence, chunks[1]);

        // recovery instructions
        let recovery = Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                "press [q] to return and try again when the digital winds are calmer",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM),
            )),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(recovery, chunks[2]);
    }
}

async fn zen_operations(tx: mpsc::UnboundedSender<ZenEvent>, args: CliArgs) {
    tokio::time::sleep(Duration::from_millis(100)).await;

    // scanning phase
    let _ = tx.send(ZenEvent::StartScanning);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // find storage locations first to calculate total operations
    let directories = find_vscode_storage_directories();

    // calculate total operations for accurate progress
    let mut total_ops = 0;
    for _dir in &directories {
        total_ops += 1; // storage update
        if !args.no_signout {
            total_ops += 1; // database cleaning
        }
    }
    let _ = tx.send(ZenEvent::SetTotalOperations(total_ops));

    // find processes
    if !args.no_terminate {
        let discovered_processes = discover_vscode_processes();
        if discovered_processes.is_empty() {
            let _ = tx.send(ZenEvent::LogMessage("no restless processes found - digital spirits already at peace".to_string()));
        } else {
            for process_stone in discovered_processes {
                let _ = tx.send(ZenEvent::ProcessFound(process_stone));
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }

    // find storage locations
    for dir in &directories {
        let display_name = dir.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let _ = tx.send(ZenEvent::LocationFound(display_name));
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    if directories.is_empty() {
        let _ = tx.send(ZenEvent::LogMessage("no vscode installations found - digital space already pure".to_string()));
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _ = tx.send(ZenEvent::OperationComplete);
        return;
    }

    // processing phase - processes are now terminated interactively via meditation stones
    tokio::time::sleep(Duration::from_millis(50)).await;

    // update storage and clean databases
    for directory in directories {
        let display_name = directory.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // update storage
        if let Err(e) = update_vscode_storage(&directory, &tx) {
            let _ = tx.send(ZenEvent::Error(format!("Storage update failed: {}", e)));
            return;
        }
        let _ = tx.send(ZenEvent::StorageUpdated(display_name.clone()));
        tokio::time::sleep(Duration::from_millis(50)).await;

        // clean database
        if !args.no_signout {
            if let Err(e) = clean_vscode_databases(&directory, &tx) {
                let _ = tx.send(ZenEvent::Error(format!("Database cleaning failed: {}", e)));
                return;
            }
            let _ = tx.send(ZenEvent::DatabaseCleaned(display_name));
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    // completion
    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = tx.send(ZenEvent::OperationComplete);
}

fn discover_vscode_processes() -> Vec<ProcessStone> {
    use sysinfo::System;
    use crate::utils::VSCODE_PROCESSES;

    let mut stones = Vec::new();

    for (pid, process) in System::new_all().processes() {
        let cmd = process.cmd().join(" ".as_ref()).to_string_lossy().to_string();
        let name = process.name().to_string_lossy().to_string();
        let exe = process.exe().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();

        let is_vscode = VSCODE_PROCESSES.iter().any(|&vs| name.eq_ignore_ascii_case(vs))
            || cmd.contains("vscode")
            || exe.contains("microsoft vs code")
            || exe.contains("cursor")
            || exe.contains("code-insiders")
            || exe.contains("windsurf")
            || exe.contains("trae")
            || (exe.contains("code") && exe.contains("electron"));

        if is_vscode {
            stones.push(ProcessStone {
                name: name.clone(),
                pid: pid.as_u32(),
                path: exe,
                is_selected: false,
                is_terminated: false,
            });
        }
    }

    stones
}
