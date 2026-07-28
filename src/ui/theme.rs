use ratatui::style::{Color, Modifier, Style};

pub const ACCENT: Color = Color::Rgb(180, 140, 80);

pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}

pub fn selected() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn title() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub fn border() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn hint() -> Style {
    Style::default().fg(Color::Gray)
}
