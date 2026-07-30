mnemo ꕤ spaced repetition, right in your terminal

quick honest question: when's the last time you actually sat down and learned something for you?

not the framework docs for work. not the side project backlog. the language you keep meaning to pick up, the stuff you wanted to have memorized by now, the random rabbit hole you fell into once and never went back to.

it's easy to let all of that quietly disappear between tabs and terminals 😭 mnemo is one small way to pull a sliver of it back — a flashcard app that lives right where you already spend your day, so reviewing a few cards is as easy as opening a new pane ✌️

## Motive

- **low friction beats good intentions.** if studying means opening another app, it mostly doesn't happen. if it's one command away from your shell, it does.
- **quiet by design.** no dashboards, no notifications, no color for color's sake — just cards, a prompt, and you.
- **built on real memory research.** mnemo uses FSRS, the modern successor to the older SM-2 algorithm — it models how *you specifically* forget things, so it stops nagging you about cards you already know cold.

## Features: current

- make decks and cards with a few keystrokes
- tells you exactly what to review, and when, using FSRS spaced repetition
- cloud-synced identity via GitHub + Supabase — sign in once, stay signed in
- cloze deletion cards, tags, and lightweight markdown (`**bold**`, `*italic*`, `` `code` ``)
- import/export decks as CSV, right from the command line

## Features: V2 (aka stuff i want)

- full deck/card sync across devices, not just identity ✌️
- a stats screen — retention rate, review heatmap, forecast of what's coming due
- session notes for cards you keep getting wrong (so future-you knows why)
- multiple profiles, for when studying and hoarding niche interests need separate decks 😭

## How FSRS (spaced repetition) works

mnemo cards each carry a memory model — `stability` and `difficulty` — instead of a single fixed interval.

after reviewing a card, you rate it: **again**, **hard**, **good**, or **easy**. each rating updates the model:

- **again** = you forgot it. stability resets, you'll see it again soon.
- **hard / good / easy** = you remembered it, to varying degrees of ease. the next interval grows accordingly — smaller for hard, larger for easy — computed fresh each time from your actual review history, not a fixed multiplier.

the practical effect: cards you're shaky on show up constantly, cards you've clearly mastered fade into the background. it's adaptive in a way a fixed "review every 3 days" schedule never could be.

## Get it running

```bash
git clone https://github.com/saiharshilk/mnemo.git
cd mnemo
cargo build --release
cargo run
```

**Requirements:** Rust stable (2021 edition or later), a terminal with UTF-8 support.

first launch drops you on the welcome screen and walks you through GitHub device-flow login (your browser opens automatically to the verification URL). once signed in, your session is cached locally and future launches skip straight to your decks.

your data lives here:

| platform | path |
|---|---|
| Linux | `~/.local/share/mnemo/{data.db,session.json}` |
| macOS | `~/Library/Application Support/mnemo/{data.db,session.json}` |
| Windows | `%LOCALAPPDATA%\mnemo\{data.db,session.json}` |

## One-time setup (auth)

mnemo needs a GitHub OAuth App and a Supabase project before your first login. yes, it's a little setup — but it's the price of not losing your account to a browser cache clear. 😭

**1. configure secrets**

```bash
cp .env.example .env
```

edit `.env` and fill in:
- `GITHUB_CLIENT_ID` — from a GitHub OAuth App with device flow enabled (below)
- `SUPABASE_URL` — e.g. `https://abcdefgh.supabase.co`
- `SUPABASE_ANON_KEY` — the anon public key from Project Settings → API

**2. create a GitHub OAuth App**

- go to [github.com/settings/applications/new](https://github.com/settings/applications/new)
- application name: anything (`mnemo` works)
- homepage URL: anything (e.g. `http://localhost:8080`) — never actually contacted
- authorization callback URL: anything — device flow doesn't use it
- after creating it, scroll to **Device Flow** and make sure it's enabled (on by default for new apps)
- copy the **Client ID** into `.env`. no client secret needed — device flow doesn't use one.

**3. create the Supabase `users` table**

from the Supabase SQL editor:

```sql
create table if not exists public.users (
    github_id        bigint primary key,
    github_username  text not null,
    avatar_url       text,
    created_at       timestamptz not null default now()
);

-- Supabase tables default to RLS enabled, which blocks the anon upsert from
-- the TUI. either disable RLS for this one table:
alter table public.users disable row level security;

-- ...or keep RLS on and add an explicit policy that allows anon upsert:
-- create policy "anon upsert users" on public.users
--   for all to anon
--   using (true)
--   with check (true);
```

mnemo calls `POST {SUPABASE_URL}/rest/v1/users` with `resolution=merge-duplicates` on every login, so this row is created on first sign-in and refreshed on every one after. the table has to exist before your first login — the app won't create it for you.

## Guide

| where | keys |
|---|---|
| welcome | `enter` log in with github · `q` quit |
| device auth | `enter` open browser to verification URL · `esc` cancel |
| auth error | `r` retry · `esc` back to welcome |
| deck list | `enter` open · `n` new deck · `e` rename · `d` delete (twice to confirm) · `s` stats · `esc`/`q` quit |
| inside a deck | `n` new card · `enter`/`e` edit · `d` delete (twice) · `r` review · `t` filter by tag · `esc` back |
| new / edit card | type front, `enter`, type back, `enter`, type tags (optional), `enter` to save · `esc` cancels |
| review | `space`/`enter` flip · then rate: `1` again · `2` hard · `3` good · `4` easy |

new cards enter the review queue immediately. a card is due when it has no review state yet, or its `due_date` has passed.

## Test

```bash
cargo test
```

scheduler tests verify the FSRS integration end to end — "again" schedules soon, "easy" grows the interval, and all four ratings return a distinct predicted interval.

## Built with

- Rust
- ratatui + crossterm — the TUI itself
- rusqlite — local SQLite storage
- rs-fsrs — FSRS scheduling, no ML framework bloat
- chrono — timestamps
- clap — CLI parsing
- ureq — sync HTTP for GitHub device flow + Supabase REST
- dotenvy — local `.env` loading
- open — launches your system browser for login

## Got questions or suggestions?

tell me now. saiharshilk@gmail.com
