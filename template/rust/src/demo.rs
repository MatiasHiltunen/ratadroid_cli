//! Demo app for Ratadroid template
//!
//! This is a simple example showing how to use the Ratadroid template.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use crossterm::event::{Event as CrosstermEvent, KeyCode};

use crate::{RatadroidApp, RatadroidContext, set_app_factory};

/// Demo TUI application
pub struct DemoApp {
    counter: i32,
}

impl DemoApp {
    pub fn new() -> Self {
        Self { counter: 0 }
    }
}

impl RatadroidApp for DemoApp {
    fn name(&self) -> &str {
        "Ratadroid Demo"
    }

    fn init(&mut self, ctx: &RatadroidContext) -> anyhow::Result<()> {
        log::info!("Demo app initialized, data dir: {:?}", ctx.data_dir);
        Ok(())
    }

    fn draw(&mut self, frame: &mut ratatui::Frame, _ctx: &RatadroidContext) {
        let area = frame.area();
        
        let text = format!(
            "Welcome to Ratadroid!\n\n\
             Counter: {}\n\n\
             Tap UP/DOWN or use on-screen keyboard to change counter.\n\
             Press ESC to quit.",
            self.counter
        );
        
        let paragraph = Paragraph::new(text)
            .block(Block::default()
                .title("Demo App")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)))
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center);
        
        frame.render_widget(paragraph, area);
    }

    fn handle_event(&mut self, event: CrosstermEvent, ctx: &mut RatadroidContext) -> bool {
        if let CrosstermEvent::Key(key) = event {
            match key.code {
                KeyCode::Esc => {
                    ctx.quit();
                    return true;
                }
                KeyCode::Up => {
                    self.counter += 1;
                    ctx.request_redraw();
                    return true;
                }
                KeyCode::Down => {
                    self.counter -= 1;
                    ctx.request_redraw();
                    return true;
                }
                _ => {}
            }
        }
        false
    }
}

/// Register the demo app factory
/// Call this before the Android main loop starts
pub fn register_demo_app() {
    set_app_factory(|| Box::new(DemoApp::new()));
}

