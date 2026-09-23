mod anki_export;
mod app;
mod auth;
mod cli;
mod csv_import;
mod db;
mod events;
mod fsrs;
mod paths;
mod text;
mod ui;

use anyhow::{Context, Result};
use app::App;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use db::open_connection;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "mnemo", about = "Terminal spaced-repetition flashcards")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Import {
        path: std::path::PathBuf,
    },
    Export {
        #[arg(long)]
        anki: bool,
        deck_name: String,
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load secrets from .env if present; ignore when the file is missing.
    let _ = dotenvy::dotenv();

    if let Some(command) = cli.command {
        match command {
            Command::Import { path } => return cli::run_import(&path),
            Command::Export {
                anki,
                deck_name,
                output,
            } => {
                if anki {
                    return cli::run_anki_export(&deck_name, output.as_deref());
                }
                let output = output.context("--output is required for generic CSV export")?;
                return cli::run_export(&deck_name, &output);
            }
        }
    }

    let conn = open_connection()?;
    let mut app = App::new(conn)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.process_event(key)?;
            }
        }

        // Drain any pending device-flow results between keystrokes.
        app.poll_auth_updates()?;

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
