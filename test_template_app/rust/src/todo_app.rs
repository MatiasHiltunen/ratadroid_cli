//! Todo App - A simple todo list application using Ratatui

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use tui_textarea::{Input, Key, TextArea};

#[derive(Clone, Debug)]
pub struct Todo {
    pub id: usize,
    pub text: String,
    pub completed: bool,
}

pub enum InputMode {
    Normal,
    Insert,
}

pub struct TodoApp {
    pub todos: Vec<Todo>,
    pub selected_index: usize,
    pub input_mode: InputMode,
    pub textarea: TextArea<'static>, // Use tui-textarea for text input
    pub next_id: usize,
    pub button_focused: usize, // Index of focused button (0 = Add, 1 = Delete, 2 = Toggle)
    pub button_clicked: Option<usize>, // Index of clicked button (for visual feedback)
    list_state: ratatui::widgets::ListState,
}

impl TodoApp {
    pub fn new() -> Self {
        Self {
            todos: vec![
                Todo {
                    id: 0,
                    text: "Welcome to Ratatui Todo App!".to_string(),
                    completed: false,
                },
                Todo {
                    id: 1,
                    text: "Press 'a' to add a new todo".to_string(),
                    completed: false,
                },
                Todo {
                    id: 2,
                    text: "Press 'd' to delete selected todo".to_string(),
                    completed: false,
                },
                Todo {
                    id: 3,
                    text: "Press Space to toggle completion".to_string(),
                    completed: false,
                },
                Todo {
                    id: 4,
                    text: "Use arrow keys to navigate".to_string(),
                    completed: false,
                },
            ],
            selected_index: 0,
            input_mode: InputMode::Normal,
            textarea: TextArea::default(),
            next_id: 5,
            button_focused: 0,
            button_clicked: None,
            list_state: {
                let mut state = ratatui::widgets::ListState::default();
                state.select(Some(0));
                state
            },
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        match &self.input_mode {
            InputMode::Normal => self.handle_normal_mode(event),
            InputMode::Insert => self.handle_insert_mode(event),
        }
    }

    fn handle_normal_mode(&mut self, event: &Event) -> bool {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                ..
            }) => {
                return true; // Quit
            }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('a'),
                        ..
                    }) => {
                        self.input_mode = InputMode::Insert;
                        self.textarea = TextArea::default(); // Reset textarea
                    }
            Event::Key(KeyEvent {
                code: KeyCode::Char('d'),
                ..
            }) => {
                self.delete_selected();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char(' '),
                ..
            }) => {
                self.toggle_selected();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                ..
            }) => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                ..
            }) => {
                if self.selected_index < self.todos.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                ..
            }) => {
                // Navigate buttons left
                if self.button_focused > 0 {
                    self.button_focused -= 1;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                ..
            }) => {
                // Navigate buttons right
                if self.button_focused < 2 {
                    self.button_focused += 1;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                ..
            }) => {
                // Only activate button if not in insert mode
                if matches!(self.input_mode, InputMode::Normal) {
                    self.activate_button(self.button_focused);
                }
            }
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: _,
                row: _,
                ..
            }) => {
                // Mouse clicks on buttons are handled in lib.rs via handle_mouse_click()
                // Mouse clicks on todo items can toggle them - handled separately if needed
                // For now, do nothing here to avoid double-triggering
            }
            _ => {}
        }
        false
    }

    fn activate_button(&mut self, button_index: usize) {
        self.button_clicked = Some(button_index);
        match button_index {
            0 => {
                // Add button - enter insert mode
                self.input_mode = InputMode::Insert;
                self.textarea = TextArea::default(); // Reset textarea
            }
            1 => {
                // Delete button - delete selected todo
                self.delete_selected();
            }
            2 => {
                // Toggle button - toggle completion of selected todo
                self.toggle_selected();
            }
            _ => {}
        }
        // button_clicked will be cleared after rendering for visual feedback
    }

    fn handle_insert_mode(&mut self, event: &Event) -> bool {
        // Convert Crossterm Event to tui-textarea Input
        let textarea_input = match event {
            Event::Key(key_event) => {
                let key = match key_event.code {
                    KeyCode::Char(c) => Key::Char(c),
                    KeyCode::Backspace => Key::Backspace,
                    KeyCode::Enter => Key::Enter,
                    KeyCode::Left => Key::Left,
                    KeyCode::Right => Key::Right,
                    KeyCode::Up => Key::Up,
                    KeyCode::Down => Key::Down,
                    KeyCode::Home => Key::Home,
                    KeyCode::End => Key::End,
                    KeyCode::PageUp => Key::PageUp,
                    KeyCode::PageDown => Key::PageDown,
                    KeyCode::Tab => Key::Tab,
                    KeyCode::Delete => Key::Delete,
                    KeyCode::Esc => Key::Esc,
                    _ => return false, // Unsupported key
                };
                
                Some(Input {
                    key,
                    ctrl: key_event.modifiers.contains(KeyModifiers::CONTROL),
                    alt: key_event.modifiers.contains(KeyModifiers::ALT),
                    shift: key_event.modifiers.contains(KeyModifiers::SHIFT),
                })
            }
            _ => None,
        };
        
        if let Some(input) = textarea_input {
            // Handle Esc or Back button to exit insert mode (discard changes)
            if input.key == Key::Esc {
                self.textarea = TextArea::default(); // Reset textarea
                self.input_mode = InputMode::Normal;
                return false;
            }
            
            // Handle Enter to confirm and add todo
            if input.key == Key::Enter && !input.ctrl && !input.alt {
                let text = self.textarea.lines().join("\n").trim().to_string();
                if !text.is_empty() {
                    self.add_todo();
                }
                self.textarea = TextArea::default(); // Reset textarea
                self.input_mode = InputMode::Normal;
                return false;
            }
            
            // Pass input to textarea
            self.textarea.input(input);
        }
        
        false
    }

    fn add_todo(&mut self) {
        let text = self.textarea.lines().join("\n").trim().to_string();
        if !text.is_empty() {
            self.todos.push(Todo {
                id: self.next_id,
                text,
                completed: false,
            });
            self.next_id += 1;
            self.selected_index = self.todos.len() - 1;
            self.list_state.select(Some(self.selected_index));
        }
    }

    fn delete_selected(&mut self) {
        if !self.todos.is_empty() {
            self.todos.remove(self.selected_index);
            if self.selected_index >= self.todos.len() && !self.todos.is_empty() {
                self.selected_index = self.todos.len() - 1;
            }
            if self.todos.is_empty() {
                self.selected_index = 0;
            }
        }
    }

    fn toggle_selected(&mut self) {
        if let Some(todo) = self.todos.get_mut(self.selected_index) {
            todo.completed = !todo.completed;
        }
    }

    pub fn render_frame(&mut self, frame: &mut Frame, area: Rect) {
        use super::button::Button;
        
        // Debug: Log dimensions
        log::info!("Render area: {}x{} (x:{}, y:{})", area.width, area.height, area.x, area.y);
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Todo list
                Constraint::Length(5), // Buttons
            ])
            .split(area);
        
        // Debug: Log chunk dimensions
        log::info!("Header chunk: {}x{} (x:{}, y:{})", chunks[0].width, chunks[0].height, chunks[0].x, chunks[0].y);
        log::info!("Todo list chunk: {}x{} (x:{}, y:{})", chunks[1].width, chunks[1].height, chunks[1].x, chunks[1].y);
        log::info!("Buttons chunk: {}x{} (x:{}, y:{})", chunks[2].width, chunks[2].height, chunks[2].x, chunks[2].y);

        // Header - ensure text is visible
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                " Todo App with Buttons ",
                Style::default()
                    .fg(Color::Cyan)
                    .bg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Status")
            .style(Style::default().fg(Color::White).bg(Color::Black)))
        .style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .alignment(Alignment::Center);
        frame.render_widget(header, chunks[0]);

        // Todo list - ensure all items are visible with proper colors
        let items: Vec<ListItem> = self.todos.iter().enumerate().map(|(i, todo)| {
            let checkbox = if todo.completed { "[x]" } else { "[ ]" };
            let is_selected = self.selected_index == i;
            
            // Use visible colors for all items - ensure contrast
            let item_style = if is_selected {
                // Selected item: bright yellow on gray background
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Gray) // Use Gray instead of DarkGray for better visibility
                    .add_modifier(Modifier::BOLD)
            } else if todo.completed {
                // Completed item: gray text on black background
                Style::default()
                    .fg(Color::Gray)
                    .bg(Color::Black)
            } else {
                // Normal item: white text on black background
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Black)
            };
            
            let content = Line::from(vec![
                Span::styled(format!("{} {}", checkbox, todo.text), item_style),
            ]);
            ListItem::new(content).style(item_style)
        }).collect();

        // Update list state
        self.list_state.select(Some(self.selected_index));
        
        // Render todo list or textarea based on input mode
        match &mut self.input_mode {
            InputMode::Insert => {
                // Render textarea for input
                let textarea_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(0),    // Textarea
                        Constraint::Length(1),  // Instructions
                    ])
                    .split(chunks[1]);
                
                // Render the textarea widget
                // Note: tui-textarea widget() method has version compatibility issues with ratatui 0.26
                // For now, render as Paragraph but still use TextArea for input handling
                let text = self.textarea.lines().join("\n");
                let textarea_widget = Paragraph::new(text)
                    .block(Block::default().borders(Borders::ALL).title("New Todo (Type here)"))
                    .style(Style::default().fg(Color::White).bg(Color::Black))
                    .wrap(ratatui::widgets::Wrap { trim: false });
                frame.render_widget(textarea_widget, textarea_chunks[0]);
                
                // Instructions
                let instructions = Paragraph::new(Line::from(vec![
                    Span::styled("Press Enter to add, Esc to cancel", Style::default().fg(Color::Yellow)),
                ]))
                .style(Style::default().fg(Color::White).bg(Color::Black))
                .alignment(Alignment::Center);
                frame.render_widget(instructions, textarea_chunks[1]);
            }
            InputMode::Normal => {
                // Render todo list
                let todos_list = List::new(items)
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title("Todos")
                        .style(Style::default().fg(Color::White).bg(Color::Black)))
                    .highlight_style(Style::default()
                        .fg(Color::Yellow)
                        .bg(Color::Gray) // Use Gray for better visibility
                        .add_modifier(Modifier::BOLD))
                    .highlight_symbol(">> ")
                    .style(Style::default().fg(Color::White).bg(Color::Black)); // Default style ensures all items are visible
                frame.render_stateful_widget(todos_list, chunks[1], &mut self.list_state);
            }
        }

        // Buttons row with instructions
        let button_row_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Instructions
                Constraint::Length(3), // Buttons
            ])
            .split(chunks[2]);
        
        // Debug: Log button row chunk dimensions
        log::info!("Button instructions chunk: {}x{} (x:{}, y:{})", button_row_chunks[0].width, button_row_chunks[0].height, button_row_chunks[0].x, button_row_chunks[0].y);
        log::info!("Button row chunk: {}x{} (x:{}, y:{})", button_row_chunks[1].width, button_row_chunks[1].height, button_row_chunks[1].x, button_row_chunks[1].y);

        // Instructions - ensure text is visible
        let instructions = Paragraph::new(Line::from(vec![
            Span::styled("Buttons: ", Style::default().fg(Color::Yellow)),
            Span::styled("←→ nav, Enter/touch", Style::default().fg(Color::White)),
        ]))
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .alignment(Alignment::Center);
        frame.render_widget(instructions, button_row_chunks[0]);

        // Buttons - ensure they fit properly
        let button_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .split(button_row_chunks[1]);
        
        // Debug: Log individual button dimensions
        log::info!("Add button chunk: {}x{} (x:{}, y:{})", button_chunks[0].width, button_chunks[0].height, button_chunks[0].x, button_chunks[0].y);
        log::info!("Delete button chunk: {}x{} (x:{}, y:{})", button_chunks[1].width, button_chunks[1].height, button_chunks[1].x, button_chunks[1].y);
        log::info!("Toggle button chunk: {}x{} (x:{}, y:{})", button_chunks[2].width, button_chunks[2].height, button_chunks[2].x, button_chunks[2].y);

        let mut add_button = Button::new("Add");
        add_button.set_focused(self.button_focused == 0);
        add_button.set_clicked(self.button_clicked == Some(0));
        add_button.render(frame, button_chunks[0]);

        let mut delete_button = Button::new("Delete");
        delete_button.set_focused(self.button_focused == 1);
        delete_button.set_clicked(self.button_clicked == Some(1));
        delete_button.render(frame, button_chunks[1]);

        let mut toggle_button = Button::new("Toggle");
        toggle_button.set_focused(self.button_focused == 2);
        toggle_button.set_clicked(self.button_clicked == Some(2));
        toggle_button.render(frame, button_chunks[2]);
    }

    /// Handle mouse/touch click - checks if click is on a button
    /// Returns true if a button was clicked, false otherwise
    pub fn handle_mouse_click(&mut self, x: u16, y: u16, area: Rect) -> bool {
        use super::button::Button;
        
        // If in insert mode, check if click is outside textarea area
        if matches!(self.input_mode, InputMode::Insert) {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Header
                    Constraint::Min(0),    // Todo list / Textarea
                    Constraint::Length(5), // Buttons
                ])
                .split(area);
            
            let textarea_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(0),    // Textarea
                    Constraint::Length(1),  // Instructions
                ])
                .split(chunks[1]);
            
            // Check if click is outside textarea area
            let textarea_rect = textarea_chunks[0];
            if !(textarea_rect.x <= x && x < textarea_rect.x + textarea_rect.width &&
                 textarea_rect.y <= y && y < textarea_rect.y + textarea_rect.height) {
                // Click outside textarea - exit insert mode
                let text = self.textarea.lines().join("\n").trim().to_string();
                if !text.is_empty() {
                    self.add_todo();
                }
                self.textarea = TextArea::default();
                self.input_mode = InputMode::Normal;
                return true;
            }
            // Click inside textarea - let textarea handle it (for now, do nothing)
            return false;
        }
        
        // Normal mode - handle button clicks and todo selection
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Todo list
                Constraint::Length(5), // Buttons
            ])
            .split(area);

        let button_row_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Instructions
                Constraint::Length(3), // Buttons
            ])
            .split(chunks[2]);

        let button_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .split(button_row_chunks[1]);

        let button = Button::new("");
        
        // Check each button and activate if clicked
        if button.contains(x, y, button_chunks[0]) {
            self.button_focused = 0; // Focus the clicked button
            self.activate_button(0);
            return true;
        }
        if button.contains(x, y, button_chunks[1]) {
            self.button_focused = 1; // Focus the clicked button
            self.activate_button(1);
            return true;
        }
        if button.contains(x, y, button_chunks[2]) {
            self.button_focused = 2; // Focus the clicked button
            self.activate_button(2);
            return true;
        }
        
        // Click was not on a button - could be on todo list
        // Check if click is on todo list area
        if y >= chunks[1].y && y < chunks[1].y + chunks[1].height {
            // Click is in todo list area - could select a todo item
            // Calculate which todo item was clicked
            let list_y = y.saturating_sub(chunks[1].y + 1); // +1 for border
            let todo_index = list_y as usize;
            if todo_index < self.todos.len() {
                self.selected_index = todo_index;
                // Optionally toggle on click
                self.toggle_selected();
                return true;
            }
        }
        
        false
    }

    pub fn render(&self, _area: Rect) -> Vec<Line> {
        let mut lines = Vec::new();

        // Title
        lines.push(Line::from(vec![
            Span::styled(
                " Todo App ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        lines.push(Line::from(""));

        // Mode indicator
        let mode_text = match self.input_mode {
            InputMode::Normal => "NORMAL",
            InputMode::Insert => "INSERT",
        };
        let mode_color = match self.input_mode {
            InputMode::Normal => Color::Green,
            InputMode::Insert => Color::Yellow,
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("Mode: {} ", mode_text),
                Style::default()
                    .fg(mode_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));

        // Instructions
        match self.input_mode {
            InputMode::Normal => {
                lines.push(Line::from("Commands:"));
                lines.push(Line::from("  a - Add new todo"));
                lines.push(Line::from("  d - Delete selected"));
                lines.push(Line::from("  Space/Enter - Toggle completion"));
                lines.push(Line::from("  ↑↓ - Navigate"));
                lines.push(Line::from("  q - Quit"));
            }
            InputMode::Insert => {
                lines.push(Line::from("Enter todo text, then press Enter or Esc"));
            }
        }
        lines.push(Line::from(""));

        // Input buffer (if in insert mode) - now handled by textarea widget
        // This render method is deprecated, but kept for compatibility
        if matches!(self.input_mode, InputMode::Insert) {
            let text: String = self.textarea.lines().join("\n");
            lines.push(Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    text.clone(),
                    Style::default().fg(Color::White),
                ),
                Span::styled("_", Style::default().fg(Color::Yellow)), // Cursor
            ]));
            lines.push(Line::from(""));
        }

        // Todo list
        if self.todos.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "No todos yet. Press 'a' to add one!",
                Style::default().fg(Color::Gray),
            )]));
        } else {
            lines.push(Line::from("Todos:"));
            for (index, todo) in self.todos.iter().enumerate() {
                let is_selected = index == self.selected_index;
                let checkbox = if todo.completed { "[x]" } else { "[ ]" };
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else if todo.completed {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::White)
                };

                let text_style = if todo.completed {
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::CROSSED_OUT)
                } else {
                    Style::default().fg(Color::White)
                };

                lines.push(Line::from(vec![
                    Span::styled(checkbox, style),
                    Span::styled(" ", style),
                    Span::styled(&todo.text, text_style),
                ]));
            }
        }

        lines
    }
}

impl Default for TodoApp {
    fn default() -> Self {
        Self::new()
    }
}

