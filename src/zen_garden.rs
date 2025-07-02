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
    process::terminate_vscode_processes,
    storage::update_vscode_storage,
};

#[derive(Debug, Clone)]
pub enum ZenEvent {
    StartScanning,
    ProcessFound(String),
    LocationFound(String),
    ProcessTerminated(String),
    StorageUpdated(String),
    DatabaseCleaned(String),
    OperationComplete,
    Error(String),
    LogMessage(String),
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
    processes: Vec<String>,
    locations: Vec<String>,
    progress: f64,
    current_operation: String,
    start_time: Instant,
    breathing_phase: f64,
    water_flow: usize,
    should_quit: bool,
}

impl ZenGarden {
    pub fn new() -> Self {
        Self {
            state: ZenState::Welcome,
            events: Vec::new(),
            processes: Vec::new(),
            locations: Vec::new(),
            progress: 0.0,
            current_operation: "preparing meditation space...".to_string(),
            start_time: Instant::now(),
            breathing_phase: 0.0,
            water_flow: 0,
            should_quit: false,
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
        let tick_rate = Duration::from_millis(50);

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
                self.progress += 0.2;
            }
            ZenEvent::StorageUpdated(location) => {
                self.events.push(format!("cleansed energy patterns in {}", location));
                self.progress += 0.3;
            }
            ZenEvent::DatabaseCleaned(location) => {
                self.events.push(format!("purified data streams in {}", location));
                self.progress += 0.3;
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
        }
    }

    fn update_animations(&mut self) {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        self.breathing_phase = (elapsed * 0.5).sin() * 0.5 + 0.5;
        self.water_flow = (elapsed * 2.0) as usize % 20;
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
            Line::from("                    🪨 meditation stones"),
            Line::from("                  ○ ○ ○   ○ ○ ○   ○ ○ ○"),
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
                "press [enter] to begin your mindful journey • [q] to return to the world",
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
            .title("🪨 meditation stones")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        let stones_area = stones_block.inner(area);
        f.render_widget(stones_block, area);

        let stone_items: Vec<ListItem> = self.processes.iter()
            .map(|p| ListItem::new(format!("○ {}", p)))
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
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // scanning phase
    let _ = tx.send(ZenEvent::StartScanning);
    tokio::time::sleep(Duration::from_millis(500)).await;

    // find processes
    if !args.no_terminate {
        let processes = ["VSCode.exe", "Cursor.exe", "Code.exe"];
        for process in processes {
            let _ = tx.send(ZenEvent::ProcessFound(process.to_string()));
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    // find storage locations
    let directories = find_vscode_storage_directories();
    for dir in &directories {
        let display_name = dir.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let _ = tx.send(ZenEvent::LocationFound(display_name));
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    if directories.is_empty() {
        let _ = tx.send(ZenEvent::Error("No VSCode installations found".to_string()));
        return;
    }

    // processing phase
    tokio::time::sleep(Duration::from_millis(500)).await;

    // terminate processes
    if !args.no_terminate {
        let _ = terminate_vscode_processes(&tx);
        let processes = ["VSCode.exe", "Cursor.exe", "Code.exe"];
        for process in processes {
            let _ = tx.send(ZenEvent::ProcessTerminated(process.to_string()));
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
    }

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
        tokio::time::sleep(Duration::from_millis(600)).await;

        // clean database
        if !args.no_signout {
            if let Err(e) = clean_vscode_databases(&directory, &tx) {
                let _ = tx.send(ZenEvent::Error(format!("Database cleaning failed: {}", e)));
                return;
            }
            let _ = tx.send(ZenEvent::DatabaseCleaned(display_name));
            tokio::time::sleep(Duration::from_millis(600)).await;
        }
    }

    // completion
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = tx.send(ZenEvent::OperationComplete);
}
