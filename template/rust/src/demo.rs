//! Demo app for Ratadroid template
//!
//! This is a comprehensive demo showing how to use the Ratadroid runtime.
//! When no custom app is registered, this demo runs automatically.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Gauge, List, ListItem, Tabs};
use crossterm::event::{Event as CrosstermEvent, KeyCode, MouseEventKind};

#[cfg(target_os = "android")]
use crate::{RatadroidApp, RatadroidContext, set_app_factory};

#[cfg(not(target_os = "android"))]
use crate::{RatadroidApp, RatadroidContext, set_app_factory};

/// Demo tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DemoTab {
    Welcome,
    Counter,
    Colors,
    Input,
}

impl DemoTab {
    fn titles() -> Vec<&'static str> {
        vec!["Welcome", "Counter", "Colors", "Input"]
    }

    fn index(&self) -> usize {
        match self {
            DemoTab::Welcome => 0,
            DemoTab::Counter => 1,
            DemoTab::Colors => 2,
            DemoTab::Input => 3,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => DemoTab::Welcome,
            1 => DemoTab::Counter,
            2 => DemoTab::Colors,
            3 => DemoTab::Input,
            _ => DemoTab::Welcome,
        }
    }

    fn next(&self) -> Self {
        Self::from_index((self.index() + 1) % 4)
    }

    fn prev(&self) -> Self {
        Self::from_index((self.index() + 3) % 4)
    }
}

/// Demo TUI application
pub struct DemoApp {
    current_tab: DemoTab,
    counter: i32,
    color_index: usize,
    input_log: Vec<String>,
    tick_count: u64,
}

impl Default for DemoApp {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoApp {
    pub fn new() -> Self {
        Self {
            current_tab: DemoTab::Welcome,
            counter: 0,
            color_index: 0,
            input_log: vec![
                "Tap or use keyboard to interact".to_string(),
            ],
            tick_count: 0,
        }
    }

    fn add_log(&mut self, msg: String) {
        self.input_log.push(msg);
        // Keep only last 10 entries
        if self.input_log.len() > 10 {
            self.input_log.remove(0);
        }
    }

    fn render_welcome(&self, frame: &mut ratatui::Frame, area: Rect, ctx: &RatadroidContext) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);

        // Title
        let title = Paragraph::new("🦀 Ratadroid Demo")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
        frame.render_widget(title, chunks[0]);

        // Main content
        let info_text = vec![
            Line::from(""),
            Line::from(Span::styled("Welcome to Ratadroid!", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from("This is the demo app that runs when no"),
            Line::from("custom RatadroidApp is registered."),
            Line::from(""),
            Line::from(Span::styled("Screen Info:", Style::default().fg(Color::Green))),
            Line::from(format!("  Size: {}×{} cells", ctx.cols, ctx.rows)),
            Line::from(format!("  Orientation: {:?}", ctx.orientation)),
            Line::from(format!("  Font: {:.1}×{:.1}px", ctx.font_width, ctx.font_height)),
            Line::from(""),
            Line::from(Span::styled("Navigation:", Style::default().fg(Color::Magenta))),
            Line::from("  ← → or TAB: Switch tabs"),
            Line::from("  ESC: Quit"),
            Line::from(""),
            Line::from(Span::styled("To create your own app:", Style::default().fg(Color::Blue))),
            Line::from("  1. Implement RatadroidApp trait"),
            Line::from("  2. Call set_app_factory()"),
        ];

        let info = Paragraph::new(info_text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::NONE));
        frame.render_widget(info, chunks[1]);

        // Footer with tick counter
        let footer = Paragraph::new(format!("Ticks: {} | Data dir: {:?}", self.tick_count, ctx.data_dir.file_name().unwrap_or_default()))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(footer, chunks[2]);
    }

    fn render_counter(&self, frame: &mut ratatui::Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        // Counter value
        let counter_text = format!("Counter: {}", self.counter);
        let counter_color = if self.counter > 0 {
            Color::Green
        } else if self.counter < 0 {
            Color::Red
        } else {
            Color::Yellow
        };
        
        let counter = Paragraph::new(counter_text)
            .style(Style::default().fg(counter_color).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().title("Value").borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
        frame.render_widget(counter, chunks[0]);

        // Progress gauge (wraps at 100)
        let progress = (self.counter.abs() % 101) as u16;
        let gauge = Gauge::default()
            .block(Block::default().title("Progress").borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)))
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
            .percent(progress)
            .label(format!("{}%", progress));
        frame.render_widget(gauge, chunks[1]);

        // Instructions
        let instructions = Paragraph::new("↑/↓: Increment/Decrement | SPACE: Reset")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(instructions, chunks[2]);
    }

    fn render_colors(&self, frame: &mut ratatui::Frame, area: Rect) {
        let colors = [
            ("Red", Color::Red),
            ("Green", Color::Green),
            ("Blue", Color::Blue),
            ("Yellow", Color::Yellow),
            ("Magenta", Color::Magenta),
            ("Cyan", Color::Cyan),
            ("White", Color::White),
            ("Gray", Color::Gray),
        ];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        // Current color highlight
        let (color_name, color) = colors[self.color_index % colors.len()];
        let highlight = Paragraph::new(format!("Selected: {}", color_name))
            .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(color)));
        frame.render_widget(highlight, chunks[0]);

        // Color palette
        let color_items: Vec<ListItem> = colors
            .iter()
            .enumerate()
            .map(|(i, (name, c))| {
                let style = if i == self.color_index % colors.len() {
                    Style::default().fg(*c).add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else {
                    Style::default().fg(*c)
                };
                ListItem::new(format!("  {}  ████████", name)).style(style)
            })
            .collect();

        let color_list = List::new(color_items)
            .block(Block::default().title("Color Palette (↑/↓ to select)").borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
        frame.render_widget(color_list, chunks[1]);
    }

    fn render_input(&self, frame: &mut ratatui::Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        // Instructions
        let instructions = Paragraph::new("Tap screen or press keys to see events logged below")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
        frame.render_widget(instructions, chunks[0]);

        // Event log
        let log_items: Vec<ListItem> = self
            .input_log
            .iter()
            .rev()
            .map(|s| ListItem::new(s.as_str()).style(Style::default().fg(Color::White)))
            .collect();

        let log_list = List::new(log_items)
            .block(Block::default().title("Event Log").borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
        frame.render_widget(log_list, chunks[1]);
    }
}

impl RatadroidApp for DemoApp {
    fn name(&self) -> &str {
        "Ratadroid Demo"
    }

    fn init(&mut self, ctx: &RatadroidContext) -> anyhow::Result<()> {
        log::info!("Demo app initialized");
        log::info!("  Screen: {}x{}", ctx.cols, ctx.rows);
        log::info!("  Data dir: {:?}", ctx.data_dir);
        Ok(())
    }

    fn draw(&mut self, frame: &mut ratatui::Frame, ctx: &RatadroidContext) {
        let area = frame.area();

        // Main layout with tabs at top
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        // Tab bar
        let tab_titles: Vec<Line> = DemoTab::titles()
            .iter()
            .map(|t| Line::from(*t))
            .collect();
        
        let tabs = Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::ALL).title("Ratadroid Demo"))
            .select(self.current_tab.index())
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        frame.render_widget(tabs, chunks[0]);

        // Content area
        let content_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        let content_area = content_block.inner(chunks[1]);
        frame.render_widget(content_block, chunks[1]);

        // Render current tab content
        match self.current_tab {
            DemoTab::Welcome => self.render_welcome(frame, content_area, ctx),
            DemoTab::Counter => self.render_counter(frame, content_area),
            DemoTab::Colors => self.render_colors(frame, content_area),
            DemoTab::Input => self.render_input(frame, content_area),
        }
    }

    fn handle_event(&mut self, event: CrosstermEvent, ctx: &mut RatadroidContext) -> bool {
        match event {
            CrosstermEvent::Key(key) => {
                match key.code {
                    KeyCode::Esc => {
                        ctx.quit();
                        return true;
                    }
                    KeyCode::Tab | KeyCode::Right => {
                        self.current_tab = self.current_tab.next();
                        ctx.request_redraw();
                        return true;
                    }
                    KeyCode::BackTab | KeyCode::Left => {
                        self.current_tab = self.current_tab.prev();
                        ctx.request_redraw();
                        return true;
                    }
                    KeyCode::Up => {
                        match self.current_tab {
                            DemoTab::Counter => self.counter += 1,
                            DemoTab::Colors => {
                                self.color_index = self.color_index.saturating_sub(1);
                            }
                            _ => {}
                        }
                        ctx.request_redraw();
                        return true;
                    }
                    KeyCode::Down => {
                        match self.current_tab {
                            DemoTab::Counter => self.counter -= 1,
                            DemoTab::Colors => {
                                self.color_index += 1;
                            }
                            _ => {}
                        }
                        ctx.request_redraw();
                        return true;
                    }
                    KeyCode::Char(' ') => {
                        if self.current_tab == DemoTab::Counter {
                            self.counter = 0;
                            ctx.request_redraw();
                            return true;
                        }
                    }
                    KeyCode::Char(c) => {
                        if self.current_tab == DemoTab::Input {
                            self.add_log(format!("Key: '{}'", c));
                            ctx.request_redraw();
                            return true;
                        }
                    }
                    _ => {
                        if self.current_tab == DemoTab::Input {
                            self.add_log(format!("Key: {:?}", key.code));
                            ctx.request_redraw();
                            return true;
                        }
                    }
                }
            }
            CrosstermEvent::Mouse(mouse) => {
                if self.current_tab == DemoTab::Input {
                    let msg = match mouse.kind {
                        MouseEventKind::Down(btn) => format!("Mouse {:?} at ({}, {})", btn, mouse.column, mouse.row),
                        MouseEventKind::Up(btn) => format!("Mouse {:?} up at ({}, {})", btn, mouse.column, mouse.row),
                        MouseEventKind::Drag(btn) => format!("Drag {:?} at ({}, {})", btn, mouse.column, mouse.row),
                        MouseEventKind::Moved => format!("Move to ({}, {})", mouse.column, mouse.row),
                        MouseEventKind::ScrollDown => "Scroll Down".to_string(),
                        MouseEventKind::ScrollUp => "Scroll Up".to_string(),
                        MouseEventKind::ScrollLeft => "Scroll Left".to_string(),
                        MouseEventKind::ScrollRight => "Scroll Right".to_string(),
                    };
                    self.add_log(msg);
                    ctx.request_redraw();
                    return true;
                }
            }
            CrosstermEvent::Resize(cols, rows) => {
                if self.current_tab == DemoTab::Input {
                    self.add_log(format!("Resize: {}x{}", cols, rows));
                }
                ctx.request_redraw();
                return true;
            }
            _ => {}
        }
        false
    }

    fn on_resize(&mut self, cols: u16, rows: u16, _ctx: &RatadroidContext) {
        log::info!("Demo app resized to {}x{}", cols, rows);
    }

    fn tick(&mut self, ctx: &mut RatadroidContext) {
        self.tick_count += 1;
        // Request redraw every ~60 ticks (about once per second) to update tick counter
        if self.tick_count % 60 == 0 && self.current_tab == DemoTab::Welcome {
            ctx.request_redraw();
        }
    }
}

/// Register the demo app factory
/// Call this before the Android main loop starts
pub fn register_demo_app() {
    set_app_factory(|| Box::new(DemoApp::new()));
}
