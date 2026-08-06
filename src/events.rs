use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Back,
    Up,
    Down,
    Confirm,
    New,
    Edit,
    Delete,
    Review,
    Stats,
    Import,
    Search,
    ToggleView,
    Flip,
    Rate(u8),
    Char(char),
    Backspace,
}

pub fn map_key(event: KeyEvent) -> Option<Action> {
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        return match event.code {
            KeyCode::Char('c') => Some(Action::Quit),
            _ => None,
        };
    }

    match event.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::Up),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::Down),
        KeyCode::Enter => Some(Action::Confirm),
        KeyCode::Char('n') => Some(Action::New),
        KeyCode::Char('e') => Some(Action::Edit),
        KeyCode::Char('d') => Some(Action::Delete),
        KeyCode::Char('r') => Some(Action::Review),
        // 's' opens the Stats screen from DeckList and is ignored elsewhere.
        KeyCode::Char('s') => Some(Action::Stats),
        KeyCode::Char('i') => Some(Action::Import),
        KeyCode::Char('/') => Some(Action::Search),
        // 'v' toggles the heatmap view on the Stats screen.
        KeyCode::Char('v') => Some(Action::ToggleView),
        KeyCode::Char(' ') => Some(Action::Flip),
        KeyCode::Char('1') => Some(Action::Rate(1)),
        KeyCode::Char('2') => Some(Action::Rate(2)),
        KeyCode::Char('3') => Some(Action::Rate(3)),
        KeyCode::Char('4') => Some(Action::Rate(4)),
        KeyCode::Backspace => Some(Action::Backspace),
        KeyCode::Char(c) => Some(Action::Char(c)),
        _ => None,
    }
}

pub fn map_key_in_input(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Up => Some(Action::Up),
        KeyCode::Down => Some(Action::Down),
        KeyCode::Enter => Some(Action::Confirm),
        KeyCode::Backspace => Some(Action::Backspace),
        KeyCode::Char(c) => Some(Action::Char(c)),
        _ => None,
    }
}
