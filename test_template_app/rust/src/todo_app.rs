//! Todo App - A simple todo list application using Ratatui

use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

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
    pub input_buffer: String,
    pub next_id: usize,
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
            input_buffer: String::new(),
            next_id: 5,
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
                self.input_buffer.clear();
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
                code: KeyCode::Enter,
                ..
            }) => {
                self.toggle_selected();
            }
            _ => {}
        }
        false
    }

    fn handle_insert_mode(&mut self, event: &Event) -> bool {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Esc,
                ..
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Enter,
                ..
            }) => {
                if !self.input_buffer.trim().is_empty() {
                    self.add_todo();
                }
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                ..
            }) => {
                self.input_buffer.pop();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                ..
            }) => {
                self.input_buffer.push(*c);
            }
            _ => {}
        }
        false
    }

    fn add_todo(&mut self) {
        let text = self.input_buffer.trim().to_string();
        if !text.is_empty() {
            self.todos.push(Todo {
                id: self.next_id,
                text,
                completed: false,
            });
            self.next_id += 1;
            self.selected_index = self.todos.len() - 1;
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

    pub fn render(&self, area: Rect) -> Vec<Line> {
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

        // Input buffer (if in insert mode)
        if matches!(self.input_mode, InputMode::Insert) {
            lines.push(Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    &self.input_buffer,
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

