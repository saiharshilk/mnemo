use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use super::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardModalStep {
    Front,
    Back,
    Tags,
}

impl CardModalStep {
    pub fn label(self) -> &'static str {
        match self {
            Self::Front => "Front",
            Self::Back => "Back",
            Self::Tags => "Tags (optional)",
        }
    }
}

pub fn draw(f: &mut Frame, title: &str, step: CardModalStep, input: &str, editing: bool) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(f.area());

    let modal_title = if editing {
        format!(" Edit Card — {} ", step.label())
    } else {
        format!(" New Card — {} ", step.label())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(modal_title)
        .title_style(theme::title());

    let display = if input.is_empty() {
        format!("{}_", step.label())
    } else {
        format!("{input}_")
    };

    let para = Paragraph::new(display).block(block);
    f.render_widget(para, chunks[0]);

    let hint =
        Paragraph::new(format!("Enter next  ·  Esc cancel  ·  {}", title)).style(theme::hint());
    f.render_widget(hint, chunks[1]);
}

pub fn draw_simple_modal(f: &mut Frame, title: &str, prompt: &str, input: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(format!(" {title} "))
        .title_style(theme::title());

    let display = if input.is_empty() {
        format!("{prompt}_")
    } else {
        format!("{input}_")
    };

    let para = Paragraph::new(display).block(block);
    f.render_widget(para, chunks[0]);

    let hint = Paragraph::new("Enter save  ·  Esc cancel").style(theme::hint());
    f.render_widget(hint, chunks[1]);
}
