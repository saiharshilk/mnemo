# mnemo

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

The first launch drops you on the Welcome screen and walks you through GitHub device-flow login (your browser opens automatically to the verification URL). Once signed in, the session is cached locally and subsequent launches skip straight to your decks.

The local data directory holds both the SQLite database and the session file:

- **Linux:**  `~/.local/share/mnemo/{data.db,session.json}`
- **macOS:**  `~/Library/Application Support/mnemo/{data.db,session.json}`
- **Windows:** `%LOCALAPPDATA%\mnemo\{data.db,session.json}`

## Stage 2 setup (one-time)

Stage 2 adds a Welcome screen + GitHub device-flow login + Supabase identity. Both decks/cards and the new auth flow need configuration before first use.

### 1. Configure secrets

```bash
cp .env.example .env
```

Then edit `.env` and fill in:

- **`GITHUB_CLIENT_ID`** — from a GitHub OAuth App with **Device Flow** enabled (see below).
- **`SUPABASE_URL`** — e.g. `https://abcdefgh.supabase.co`.
- **`SUPABASE_ANON_KEY`** — anon public key from Project Settings → API.

Both keys must be present for the post-login Supabase identity upsert to run. The local SQLite database still stores all your deck/card data — Supabase is only used as an identity layer.

### 2. Create a GitHub OAuth App (with Device Flow)

1. Go to **GitHub → Settings → Developer settings → OAuth Apps → New OAuth App**:
   <https://github.com/settings/applications/new>
2. Fill in:
   - **Application name:** anything (e.g. `mnemo`)
   - **Homepage URL:** any URL (e.g. `http://localhost:8080`); never contacted
   - **Authorization callback URL:** any URL (not used by device flow)
3. After creation, on the app settings page scroll to **Device Flow** and ensure it is **enabled** (it is by default for new OAuth Apps on GitHub.com).
4. Copy the **Client ID** into `GITHUB_CLIENT_ID` in `.env`. No client secret is required — device flow does not use one.

### 3. Create the Supabase `users` table

From the Supabase dashboard SQL editor (or `psql`), run:

```sql
create table if not exists public.users (
    github_id        bigint primary key,
    github_username  text not null,
    avatar_url       text,
    created_at       timestamptz not null default now()
);

-- Supabase tables default to RLS enabled, which blocks the anon upsert from
-- the TUI. Either disable RLS for this single table:
alter table public.users disable row level security;

-- …or keep RLS on and add an explicit policy that lets anon upsert:
-- create policy "anon upsert users" on public.users
--   for all to anon
--   using (true)
--   with check (true);
```

The Rust side calls `POST {SUPABASE_URL}/rest/v1/users` with `resolution=merge-duplicates` on login, so the row is created on first sign-in and updated on every subsequent login. **The table needs to exist before first login** — the app does not create it from Rust.

## Usage

On launch you see either the **Deck List** (when a valid session is cached) or the **Welcome** screen. Navigate with arrow keys or `j`/`k`, press `Enter` to open a deck.

| Screen | Keys |
|--------|------|
| Welcome | `Enter` log in to github · `q` quit |
| Device Auth | `Enter` open browser to verification URL · `Esc` cancel |
| Auth Error | `r` retry · `Esc` back to Welcome |
| Deck List | `n` new deck · `e` rename · `d` delete (press twice) · `Enter` open · `Esc`/`q` quit |
| Deck View | `n` new card · `Enter`/`e` edit · `d` delete (press twice) · `r` review · `Esc` back |
| Review | `Space`/`Enter` flip · `1`–`4` rate (again/hard/good/easy) · `Esc` end session |
| Modals | type text · `Enter` next/save · `Esc` cancel |

New cards appear in review immediately. Due cards are those with no review state yet or a `due_date` in the past.

## Test

```bash
cargo test
```

Scheduler tests verify FSRS integration (e.g. "again" schedules soon, "easy" grows the interval, four intervals returned).

## Stack

- [ratatui](https://ratatui.rs/) + [crossterm](https://github.com/crossterm-rs/crossterm) — TUI
- [rusqlite](https://github.com/rusqlite/rusqlite) — SQLite storage
- [rs-fsrs](https://github.com/open-spaced-repetition/rs-fsrs) — FSRS scheduling
- [chrono](https://github.com/chronotope/chrono) — timestamps
- [clap](https://github.com/clap-rs/clap) — CLI parsing
- [ureq](https://github.com/algesten/ureq) — sync HTTP for GitHub device flow + Supabase REST
- [dotenvy](https://github.com/allan2/dotenvy) — local `.env` loader
- [open](https://github.com/Stebalien/rust-open) — `xdg-open`/`open` to launch the system browser
