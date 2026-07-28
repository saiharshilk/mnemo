# flashcard-tui

A terminal-based spaced-repetition flashcard app for studying without leaving the terminal. Built in Rust with [FSRS](https://github.com/open-spaced-repetition/fsrs) scheduling and a minimal "researcher" aesthetic.

## Requirements

- Rust stable (2021 edition or later)
- A terminal with UTF-8 support

## Build

```bash
cargo build --release
```

## Run

```bash
cargo run
```

The SQLite database is created automatically at:

- **Linux:** `~/.local/share/flashcard-tui/data.db`
- **macOS:** `~/Library/Application Support/flashcard-tui/data.db`
- **Windows:** `%LOCALAPPDATA%\flashcard-tui\data.db`

## Usage

On startup you see the **Deck List**. Navigate with arrow keys or `j`/`k`, press `Enter` to open a deck.

| Screen | Keys |
|--------|------|
| Deck List | `n` new deck · `e` rename · `d` delete (press twice) · `Enter` open · `Esc`/`q` quit |
| Deck View | `n` new card · `Enter`/`e` edit · `d` delete (press twice) · `r` review · `Esc` back |
| Review | `Space`/`Enter` flip · `1`–`4` rate (again/hard/good/easy) · `Esc` end session |
| Modals | type text · `Enter` next/save · `Esc` cancel |

New cards appear in review immediately. Due cards are those with no review state yet or a `due_date` in the past.

## Test

```bash
cargo test
```

Scheduler tests verify FSRS integration (e.g. "again" schedules soon, "easy" grows the interval).

## Stack

- [ratatui](https://ratatui.rs/) + [crossterm](https://github.com/crossterm-rs/crossterm) — TUI
- [rusqlite](https://github.com/rusqlite/rusqlite) — SQLite storage
- [rs-fsrs](https://github.com/open-spaced-repetition/rs-fsrs) — FSRS scheduling
- [chrono](https://github.com/chronotope/chrono) — timestamps
- [clap](https://github.com/clap-rs/clap) — CLI parsing
