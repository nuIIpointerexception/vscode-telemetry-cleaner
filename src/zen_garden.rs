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
    DetailedError(crate::utils::CleanerError),
    Warning(String),
    LogMessage(String),
    SetTotalOperations(usize),
    ErrorSummary(crate::utils::ErrorCollector),
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
    CardSelection,
    Scanning,
    Processing,
    Complete,
    Error,
}

#[derive(Debug, Clone)]
pub struct CleaningCard {
    pub name: String,
    pub description: String,
    pub is_selected: bool,
    pub card_type: CardType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CardType {
    Augment,
    Cursor,
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
    error_collector: crate::utils::ErrorCollector,
    detailed_errors: Vec<crate::utils::CleanerError>,
    warnings: Vec<String>,
    cards: Vec<CleaningCard>,
    selected_card: usize,
}

impl ZenGarden {
    pub fn new(args: &CliArgs) -> Self {
        let cards = vec![
            CleaningCard {
                name: "Augment Extension".to_string(),
                description: "Clean Augment extension data from VSCode/Cursor".to_string(),
                is_selected: args.augment,
                card_type: CardType::Augment,
            },
            CleaningCard {
                name: "Cursor IDE".to_string(),
                description: "Clean Cursor IDE telemetry and configuration".to_string(),
                is_selected: args.cursor,
                card_type: CardType::Cursor,
            },
        ];

        // determine initial state based on CLI flags
        let initial_state = if args.augment || args.cursor {
            ZenState::Scanning  // skip card selection and start immediately
        } else {
            ZenState::CardSelection  // show card selection screen
        };

        Self {
            state: initial_state,
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
            error_collector: crate::utils::ErrorCollector::new(),
            detailed_errors: Vec::new(),
            warnings: Vec::new(),
            cards,
            selected_card: 0,
        }
    }

    pub async fn run(&mut self, args: CliArgs) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let (tx, mut rx) = mpsc::unbounded_channel();

        // spawn background task for operations if CLI flags are provided
        if args.augment || args.cursor {
            let tx_clone = tx.clone();
            let args_clone = args.clone();

            // determine selected cards based on CLI flags
            let mut selected_cards = Vec::new();
            if args.augment {
                selected_cards.push(CardType::Augment);
            }
            if args.cursor {
                selected_cards.push(CardType::Cursor);
            }

            tokio::spawn(async move {
                zen_operations_with_cards(tx_clone, args_clone, selected_cards).await;
            });
        }

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
                            KeyCode::Enter => {
                                if self.state == ZenState::Welcome {
                                    self.state = ZenState::CardSelection;
                                } else if self.state == ZenState::CardSelection {
                                    // start cleaning with selected cards
                                    let selected_cards: Vec<_> = self.cards.iter()
                                        .filter(|c| c.is_selected)
                                        .map(|c| c.card_type.clone())
                                        .collect();

                                    if !selected_cards.is_empty() {
                                        self.state = ZenState::Scanning;

                                        // spawn new background task with selected cards
                                        let tx_ops = tx.clone();
                                        let args_ops = args.clone();
                                        tokio::spawn(async move {
                                            zen_operations_with_cards(tx_ops, args_ops, selected_cards).await;
                                        });
                                    }
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
                            KeyCode::Char(' ') => {
                                if self.state == ZenState::CardSelection {
                                    // toggle selected card
                                    if let Some(card) = self.cards.get_mut(self.selected_card) {
                                        card.is_selected = !card.is_selected;
                                    }
                                } else if self.state == ZenState::Scanning || self.state == ZenState::Processing {
                                    // terminate selected process (same as Enter)
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
                            KeyCode::Tab => {
                                if self.state == ZenState::CardSelection {
                                    // move to next card
                                    self.selected_card = (self.selected_card + 1) % self.cards.len();
                                }
                            }
                            KeyCode::Up => {
                                if self.state == ZenState::CardSelection {
                                    // move to previous card
                                    if self.selected_card > 0 {
                                        self.selected_card -= 1;
                                    } else {
                                        self.selected_card = self.cards.len() - 1;
                                    }
                                } else if !self.processes.is_empty() && self.selected_stone > 0 {
                                    self.selected_stone -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if self.state == ZenState::CardSelection {
                                    // move to next card
                                    self.selected_card = (self.selected_card + 1) % self.cards.len();
                                } else if !self.processes.is_empty() && self.selected_stone < self.processes.len() - 1 {
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
            ZenEvent::DetailedError(error) => {
                self.detailed_errors.push(error.clone());
                self.error_collector.add_error(error.clone());
                self.events.push(format!("turbulence detected: {}", error));
                // don't immediately switch to error state - collect errors and continue
            }
            ZenEvent::Warning(warning) => {
                self.warnings.push(warning.clone());
                self.error_collector.add_warning(warning.clone());
                self.events.push(format!("gentle warning: {}", warning));
            }
            ZenEvent::ErrorSummary(collector) => {
                self.error_collector = collector.clone();
                if collector.has_errors() {
                    self.state = ZenState::Error;
                    self.current_operation = format!("meditation disrupted - {}", collector.get_summary());
                } else if collector.has_warnings() {
                    self.events.push(format!("meditation completed with mindful observations: {}", collector.get_summary()));
                }
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
            ZenState::CardSelection => self.render_card_selection(f, inner),
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

    fn render_card_selection(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(8),
                Constraint::Length(3),
            ])
            .split(area);

        // title
        let title = Paragraph::new(Line::from(vec![
            Span::styled("🧘 ", Style::default().fg(Color::Yellow)),
            Span::styled("select cleaning modules", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(" 🧘", Style::default().fg(Color::Yellow)),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(title, chunks[0]);

        // automatic grid layout for cards
        let cards_per_row = 3; // can be adjusted for more cards later
        let card_rows = (self.cards.len() + cards_per_row - 1) / cards_per_row;

        // create row constraints
        let row_constraints: Vec<Constraint> = (0..card_rows)
            .map(|_| Constraint::Length(6)) // smaller card height
            .collect();

        let row_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(row_constraints)
            .split(chunks[1]);

        // render cards in grid
        for (card_idx, card) in self.cards.iter().enumerate() {
            let row = card_idx / cards_per_row;
            let col = card_idx % cards_per_row;

            if row < row_layout.len() {
                // create column layout for this row
                let cols_in_row = std::cmp::min(cards_per_row, self.cards.len() - row * cards_per_row);
                let col_constraints: Vec<Constraint> = (0..cols_in_row)
                    .map(|_| Constraint::Percentage(100 / cols_in_row as u16))
                    .collect();

                let col_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(col_constraints)
                    .split(row_layout[row]);

                if col < col_layout.len() {
                    self.render_cleaning_card(f, col_layout[col], card, card_idx == self.selected_card);
                }
            }
        }

        // instructions
        let selected_count = self.cards.iter().filter(|c| c.is_selected).count();
        let instruction_text = if selected_count > 0 {
            format!("space: toggle • tab: next • enter: run {} module(s) • q: quit", selected_count)
        } else {
            "space: toggle • tab: next • enter: run (select at least one) • q: quit".to_string()
        };

        let instructions = Paragraph::new(Line::from(vec![
            Span::styled(instruction_text, Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC)),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(instructions, chunks[2]);
    }

    fn render_cleaning_card(&self, f: &mut Frame, area: Rect, card: &CleaningCard, is_focused: bool) {
        let border_color = if is_focused {
            Color::Yellow
        } else if card.is_selected {
            Color::Green
        } else {
            Color::Gray
        };

        let border_style = if is_focused {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };

        // compact card title
        let short_name = match card.card_type {
            CardType::Augment => "Augment",
            CardType::Cursor => "Cursor",
        };

        let card_block = Block::default()
            .title(format!(" {} ", short_name))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color).add_modifier(border_style));

        let card_area = card_block.inner(area);
        f.render_widget(card_block, area);

        // compact content layout
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // icon line
                Constraint::Length(1), // status line
                Constraint::Length(1), // focus indicator
            ])
            .split(card_area);

        // icon
        let icon = match card.card_type {
            CardType::Augment => "🔧",
            CardType::Cursor => "🖱️",
        };

        let icon_widget = Paragraph::new(Line::from(Span::styled(
            icon,
            Style::default().add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center);
        f.render_widget(icon_widget, content_chunks[0]);

        // status
        let status_text = if card.is_selected { "✓ selected" } else { "○ not selected" };
        let status_color = if card.is_selected { Color::Green } else { Color::Gray };

        let status = Paragraph::new(Line::from(Span::styled(
            status_text,
            Style::default().fg(status_color),
        )))
        .alignment(Alignment::Center);
        f.render_widget(status, content_chunks[1]);

        // focus indicator
        if is_focused {
            let focus_indicator = Paragraph::new(Line::from(Span::styled(
                "← focused",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
            )))
            .alignment(Alignment::Center);
            f.render_widget(focus_indicator, content_chunks[2]);
        }
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
        let has_issues = self.error_collector.has_errors() || self.error_collector.has_warnings();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(if has_issues {
                [
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(4),
                    Constraint::Length(3),
                ]
            } else {
                [
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(0),
                    Constraint::Length(3),
                ]
            })
            .split(area);

        // completion message with summary
        let completion_title = if has_issues {
            format!("🌸 meditation complete - {} 🌸", self.error_collector.get_summary())
        } else {
            "🌸 digital harmony achieved 🌸".to_string()
        };

        let completion = Paragraph::new(Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                completion_title,
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

        // show issues summary if there were any
        if has_issues {
            self.render_completion_summary(f, chunks[2]);
        }

        // exit instructions
        let exit_text = Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                "press [q] to return to the world with renewed digital mindfulness",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM),
            )),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(exit_text, chunks[3]);
    }

    fn render_completion_summary(&self, f: &mut Frame, area: Rect) {
        let summary_block = Block::default()
            .title("🌊 mindful observations")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let summary_area = summary_block.inner(area);
        f.render_widget(summary_block, area);

        let mut summary_lines = Vec::new();

        if self.error_collector.has_errors() {
            summary_lines.push(format!("⚡ {} turbulent moments encountered", self.error_collector.error_count()));
        }

        if self.error_collector.has_warnings() {
            summary_lines.push(format!("🌤️ {} gentle warnings observed", self.error_collector.warning_count()));
        }

        let summary_items: Vec<ListItem> = summary_lines.iter()
            .map(|line| {
                let style = if line.contains("turbulent") {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                ListItem::new(format!("• {}", line)).style(style)
            })
            .collect();

        let summary_list = List::new(summary_items);
        f.render_widget(summary_list, summary_area);
    }

    fn render_turbulence(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(8),
                Constraint::Length(6),
                Constraint::Length(3),
            ])
            .split(area);

        // error header
        let error_summary = if self.error_collector.has_errors() {
            format!("🌪️ {} errors disrupted the meditation 🌪️", self.error_collector.error_count())
        } else {
            "🌪️ turbulence in the digital realm 🌪️".to_string()
        };

        let error_msg = Paragraph::new(Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                error_summary,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(error_msg, chunks[0]);

        // detailed error list
        self.render_error_details(f, chunks[1]);

        // event log
        self.render_error_log(f, chunks[2]);

        // recovery instructions
        let recovery = Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                "press [q] to return and try again when the digital winds are calmer",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM),
            )),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(recovery, chunks[3]);
    }

    fn render_error_details(&self, f: &mut Frame, area: Rect) {
        let error_block = Block::default()
            .title("🔥 turbulence sources")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        let error_area = error_block.inner(area);
        f.render_widget(error_block, area);

        if self.detailed_errors.is_empty() {
            let no_details = Paragraph::new(Text::from(vec![
                Line::from(""),
                Line::from("                    ⚡ unexpected energy patterns"),
                Line::from("                  have disrupted the meditation"),
                Line::from(""),
                Line::from("            🌊 breathe deeply, center yourself 🌊"),
            ]))
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
            f.render_widget(no_details, error_area);
        } else {
            let error_items: Vec<ListItem> = self.detailed_errors.iter()
                .take(6) // limit to prevent overflow
                .map(|error| {
                    ListItem::new(format!("• {}", error))
                        .style(Style::default().fg(Color::Red))
                })
                .collect();

            let error_list = List::new(error_items);
            f.render_widget(error_list, error_area);
        }
    }

    fn render_error_log(&self, f: &mut Frame, area: Rect) {
        let log_block = Block::default()
            .title("📜 meditation journal")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let log_area = log_block.inner(area);
        f.render_widget(log_block, area);

        let log_items: Vec<ListItem> = self.events.iter()
            .rev()
            .take(4)
            .map(|event| {
                let style = if event.contains("error") || event.contains("turbulence") {
                    Style::default().fg(Color::Red)
                } else if event.contains("warning") {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(format!("• {}", event)).style(style)
            })
            .collect();

        let log_list = List::new(log_items);
        f.render_widget(log_list, log_area);
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

    // update storage and clean databases - continue even if some operations fail
    let mut overall_error_collector = crate::utils::ErrorCollector::new();

    for directory in directories {
        let display_name = directory.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // update storage - continue even if this fails
        match update_vscode_storage(&directory, &tx) {
            Ok(_) => {
                let _ = tx.send(ZenEvent::StorageUpdated(display_name.clone()));
            }
            Err(e) => {
                let error = crate::utils::CleanerError::FileSystem {
                    operation: "storage update".to_string(),
                    path: directory.display().to_string(),
                    source: e.to_string(),
                };
                overall_error_collector.add_error(error.clone());
                let _ = tx.send(ZenEvent::DetailedError(error));
                // still mark as "updated" to continue progress tracking
                let _ = tx.send(ZenEvent::StorageUpdated(format!("{} (with errors)", display_name.clone())));
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;

        // clean database - continue even if this fails
        if !args.no_signout {
            match clean_vscode_databases(&directory, &tx) {
                Ok(_) => {
                    let _ = tx.send(ZenEvent::DatabaseCleaned(display_name));
                }
                Err(e) => {
                    let error = crate::utils::CleanerError::Database {
                        operation: "database cleaning".to_string(),
                        path: directory.display().to_string(),
                        source: e.to_string(),
                    };
                    overall_error_collector.add_error(error.clone());
                    let _ = tx.send(ZenEvent::DetailedError(error));
                    // still mark as "cleaned" to continue progress tracking
                    let _ = tx.send(ZenEvent::DatabaseCleaned(format!("{} (with errors)", display_name)));
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    // send final error summary if there were any errors
    if overall_error_collector.has_errors() {
        let _ = tx.send(ZenEvent::ErrorSummary(overall_error_collector));
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

        let name_lower = name.to_lowercase();
        let cmd_lower = cmd.to_lowercase();
        let exe_lower = exe.to_lowercase();

        let is_vscode = VSCODE_PROCESSES.iter().any(|&vs| name.eq_ignore_ascii_case(vs))
            || cmd_lower.contains("vscode")
            || exe_lower.contains("microsoft vs code")
            || exe_lower.contains("visual studio code")
            || name_lower.contains("cursor")
            || name_lower.contains("code-insiders")
            || name_lower.contains("windsurf")
            || name_lower.contains("trae")
            || name_lower.contains("vscodium")
            || exe_lower.contains("/code")
            || exe_lower.contains("\\code.exe")
            || exe_lower.contains("/cursor")
            || exe_lower.contains("\\cursor.exe")
            || (exe_lower.contains("code") && exe_lower.contains("electron"))
            || exe_lower.contains(".app/contents/macos/electron");

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

async fn zen_operations_with_cards(tx: mpsc::UnboundedSender<ZenEvent>, _args: CliArgs, selected_cards: Vec<CardType>) {
    tokio::time::sleep(Duration::from_millis(100)).await;

    // scanning phase
    let _ = tx.send(ZenEvent::StartScanning);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut total_operations = 0;
    let mut _completed_operations = 0;

    // determine what operations we need to do
    let do_augment = selected_cards.contains(&CardType::Augment);
    let do_cursor = selected_cards.contains(&CardType::Cursor);

    if do_augment {
        total_operations += 3; // processes, storage, database
    }
    if do_cursor {
        total_operations += 2; // processes, config
    }

    let _ = tx.send(ZenEvent::SetTotalOperations(total_operations));

    // process augment cleaning
    if do_augment {
        let _ = tx.send(ZenEvent::LogMessage("beginning augment extension purification...".to_string()));

        match crate::augment::clean_augment_extension(&_args).await {
            Ok(result) => {
                for process in result.processes_terminated {
                    let _ = tx.send(ZenEvent::ProcessTerminated(process));
                }
                for dir in result.directories_found {
                    let _ = tx.send(ZenEvent::LocationFound(dir.to_string_lossy().to_string()));
                }
                for storage in result.storage_updated {
                    let _ = tx.send(ZenEvent::StorageUpdated(storage));
                }
                for db in result.databases_cleaned {
                    let _ = tx.send(ZenEvent::DatabaseCleaned(db));
                }

                if result.errors.has_errors() {
                    let _ = tx.send(ZenEvent::ErrorSummary(result.errors));
                }

                _completed_operations += 3;
            }
            Err(e) => {
                let _ = tx.send(ZenEvent::Error(format!("augment cleaning failed: {}", e)));
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // process cursor cleaning
    if do_cursor {
        let _ = tx.send(ZenEvent::LogMessage("beginning cursor ide purification...".to_string()));

        match crate::cursor::clean_cursor_ide(&_args).await {
            Ok(result) => {
                for process in result.processes_terminated {
                    let _ = tx.send(ZenEvent::ProcessTerminated(process));
                }
                for dir in result.directories_removed {
                    let _ = tx.send(ZenEvent::LocationFound(dir.to_string_lossy().to_string()));
                }

                if result.config_updated {
                    let _ = tx.send(ZenEvent::StorageUpdated("cursor configuration".to_string()));
                }

                if result.errors.has_errors() {
                    let _ = tx.send(ZenEvent::ErrorSummary(result.errors));
                }

                _completed_operations += 2;
            }
            Err(e) => {
                let _ = tx.send(ZenEvent::Error(format!("cursor cleaning failed: {}", e)));
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // completion
    let _ = tx.send(ZenEvent::LogMessage("digital purification complete - mind at peace".to_string()));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = tx.send(ZenEvent::OperationComplete);
}
