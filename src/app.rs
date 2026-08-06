use crate::auth::{AuthUpdate, Session, github, supabase};
use crate::csv_import::CsvPreview;
use crate::db::{self, CardWithReview, Deck, DeckSummary};
use crate::events::{Action, map_key, map_key_in_input};
use crate::fsrs::scheduler::schedule;
use crate::ui::CardModalStep;
use anyhow::Result;
use chrono::{NaiveDate, Utc};
use rs_fsrs::Rating;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    DeviceAuth {
        user_code: String,
        verification_uri: String,
    },
    AuthError {
        message: String,
    },
    DeckList,
    DeckView {
        deck_id: i64,
    },
    Review {
        deck_id: i64,
        cram: bool,
    },
    CardModal {
        deck_id: i64,
        editing: Option<i64>,
    },
    NewDeckModal,
    RenameDeckModal {
        deck_id: i64,
    },
    Stats,
    ImportCsv,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStep {
    FilePath,
    Preview,
    DeckChoice,
    NewDeckName,
    ExistingDeck,
    Confirm,
}

impl ImportStep {
    pub fn title(self) -> &'static str {
        match self {
            Self::FilePath => "File",
            Self::Preview => "Preview",
            Self::DeckChoice => "Deck",
            Self::NewDeckName => "New deck",
            Self::ExistingDeck => "Deck",
            Self::Confirm => "Confirm",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::FilePath | Self::NewDeckName => "Enter continue  ·  Esc back",
            Self::Preview => "Enter choose deck  ·  Esc change path",
            Self::DeckChoice => "↑↓/jk select  Enter choose  Esc back  q quit",
            Self::ExistingDeck => "↑↓/jk select  Enter choose  Esc back  q quit",
            Self::Confirm => "Enter confirm import  ·  Esc cancel  q quit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HeatmapView {
    #[default]
    Daily,
    Weekly,
}

#[derive(Debug, Clone)]
pub struct StatsState {
    pub retention: Option<f64>,
    pub heatmap: Vec<(NaiveDate, i64)>,
    pub forecast: Vec<(NaiveDate, i64)>,
    pub view: HeatmapView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeleteTarget {
    Deck(i64),
    Card(i64),
}

pub struct App {
    pub conn: Connection,
    pub should_quit: bool,
    pub screen_stack: Vec<Screen>,

    pub session: Option<Session>,

    pub decks: Vec<DeckSummary>,
    pub deck_list_selected: usize,

    pub current_deck: Option<Deck>,
    pub cards: Vec<CardWithReview>,
    pub deck_view_selected: usize,

    pub review_queue: Vec<CardWithReview>,
    pub review_index: usize,
    pub review_flipped: bool,
    pub review_message: Option<String>,

    pub input_buffer: String,
    pub card_modal_step: CardModalStep,
    pub card_draft_front: String,
    pub card_draft_back: String,

    delete_pending: Option<DeleteTarget>,
    pub delete_pending_at: Option<Instant>,

    auth_rx: Option<mpsc::Receiver<AuthUpdate>>,
    auth_cancel: Option<std::sync::Arc<AtomicBool>>,

    pub stats_state: Option<StatsState>,

    pub import_step: ImportStep,
    pub import_preview: Option<CsvPreview>,
    pub import_decks: Vec<(i64, String)>,
    pub import_selected: usize,
    pub import_deck_name: String,
    pub import_error: Option<String>,
    pub import_status: Option<String>,
    import_status_at: Option<Instant>,
    import_path: Option<PathBuf>,

    pub search_query: String,
    pub search_results: Vec<(crate::db::Card, String)>,
    pub search_selected: usize,

    pub deck_view_message: Option<String>,
}

impl App {
    pub fn new(conn: Connection) -> Result<Self> {
        let (session, stack_top) = Self::load_session()?;

        let mut app = Self {
            conn,
            should_quit: false,
            session,
            screen_stack: vec![stack_top],
            decks: Vec::new(),
            deck_list_selected: 0,
            current_deck: None,
            cards: Vec::new(),
            deck_view_selected: 0,
            review_queue: Vec::new(),
            review_index: 0,
            review_flipped: false,
            review_message: None,
            input_buffer: String::new(),
            card_modal_step: CardModalStep::Front,
            card_draft_front: String::new(),
            card_draft_back: String::new(),
            delete_pending: None,
            delete_pending_at: None,
            auth_rx: None,
            auth_cancel: None,
            stats_state: None,
            import_step: ImportStep::FilePath,
            import_preview: None,
            import_decks: Vec::new(),
            import_selected: 0,
            import_deck_name: String::new(),
            import_error: None,
            import_status: None,
            import_status_at: None,
            import_path: None,
            search_query: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            deck_view_message: None,
        };

        if app.session.is_some() {
            app.refresh_decks()?;
        }
        Ok(app)
    }

    /// Loads a persisted session at startup and decides which screen to show.
    /// - 200 from GitHub -> keep session, start on DeckList
    /// - 401 from GitHub -> clear session, start on Welcome
    /// - network failure -> keep session, warn on stderr, start on DeckList
    fn load_session() -> Result<(Option<Session>, Screen)> {
        let path = match crate::db::session_path() {
            Ok(p) => p,
            Err(_) => {
                eprintln!("auth: no session file found");
                return Ok((None, Screen::Welcome));
            }
        };

        if !path.exists() {
            eprintln!("auth: no session file found");
            return Ok((None, Screen::Welcome));
        }

        let session = match Session::load()? {
            Some(s) => s,
            None => {
                let _ = std::fs::remove_file(&path);
                eprintln!("auth: session file malformed, clearing");
                return Ok((None, Screen::Welcome));
            }
        };

        let session = match github::check_token(&session.github_token) {
            github::TokenStatus::Valid => {
                eprintln!("auth: session found, token valid, skipping login");
                return Ok((Some(session), Screen::DeckList));
            }
            github::TokenStatus::Invalid => {
                eprintln!("auth: session found, token invalid, clearing");
                let _ = std::fs::remove_file(&path);
                None
            }
            github::TokenStatus::Inconclusive(e) => {
                eprintln!("auth: session found, network check failed, proceeding anyway: {e}");
                Some(session)
            }
        };

        Ok((session, Screen::Welcome))
    }

    pub fn current_screen(&self) -> &Screen {
        self.screen_stack.last().expect("screen stack never empty")
    }

    pub fn review_current(&self) -> Option<&CardWithReview> {
        if self.review_message.is_some() {
            return None;
        }
        self.review_queue.get(self.review_index)
    }

    pub fn delete_pending(&self) -> bool {
        self.delete_pending.is_some()
    }

    pub fn handle_key(&mut self, action: Action) -> Result<()> {
        if self.is_input_screen() {
            return self.handle_input_action(action);
        }
        if self.is_auth_screen() {
            return self.handle_auth_screen_action(action);
        }

        match action {
            Action::Quit => self.should_quit = true,
            Action::Back => self.pop_screen()?,
            Action::Up => self.move_selection(-1),
            Action::Down => self.move_selection(1),
            Action::Confirm => {
                if matches!(self.current_screen(), Screen::Review { .. }) {
                    self.handle_flip();
                } else {
                    self.handle_confirm()?;
                }
            }
            Action::New => self.handle_new()?,
            Action::Edit => match self.current_screen().clone() {
                Screen::DeckList => self.handle_rename()?,
                Screen::DeckView { .. } => self.handle_edit()?,
                _ => {}
            },
            Action::Delete => self.handle_delete()?,
            Action::Review => self.handle_start_review(false)?,
            Action::Cram => self.handle_start_review(true)?,
            Action::Stats => self.handle_stats()?,
            Action::Import => self.start_import()?,
            Action::Search => self.start_search()?,
            Action::ToggleView => self.handle_toggle_view(),
            Action::Flip => self.handle_flip(),
            Action::Rate(n) => self.handle_rate(n)?,
            Action::Char(_) | Action::Backspace => {}
        }
        Ok(())
    }

    fn is_input_screen(&self) -> bool {
        matches!(
            self.current_screen(),
            Screen::CardModal { .. }
                | Screen::NewDeckModal
                | Screen::RenameDeckModal { .. }
                | Screen::ImportCsv
                | Screen::Search
        )
    }

    fn is_auth_screen(&self) -> bool {
        matches!(
            self.current_screen(),
            Screen::Welcome | Screen::DeviceAuth { .. } | Screen::AuthError { .. }
        )
    }

    fn handle_auth_screen_action(&mut self, action: Action) -> Result<()> {
        match (self.current_screen().clone(), action) {
            (Screen::Welcome, Action::Confirm) => self.start_device_flow()?,
            (_, Action::Quit) => self.should_quit = true,
            (Screen::DeviceAuth { .. }, Action::Confirm) => self.open_verification_url(),
            (Screen::AuthError { .. }, Action::Review) => self.retry_auth()?,
            (_, Action::Back) => self.handle_auth_back()?,
            _ => {}
        }
        Ok(())
    }

    fn handle_input_action(&mut self, action: Action) -> Result<()> {
        if matches!(self.current_screen(), Screen::ImportCsv) {
            return self.handle_import_input(action);
        }
        if matches!(self.current_screen(), Screen::Search) {
            return self.handle_search_input(action);
        }
        match action {
            Action::Back => self.pop_screen()?,
            Action::Confirm => self.submit_input()?,
            Action::Char(c) => self.input_buffer.push(c),
            Action::Backspace => {
                self.input_buffer.pop();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_search_input(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Back => self.pop_screen()?,
            Action::Confirm => self.open_search_result()?,
            Action::Char(c) => {
                self.search_query.push(c);
                self.refresh_search()?;
            }
            Action::Backspace => {
                self.search_query.pop();
                self.refresh_search()?;
            }
            Action::Up => self.move_search_selection(-1),
            Action::Down => self.move_search_selection(1),
            _ => {}
        }
        Ok(())
    }

    fn handle_import_input(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Back => self.import_back()?,
            Action::Confirm => self.import_confirm()?,
            Action::Char(c) => {
                if matches!(
                    self.import_step,
                    ImportStep::FilePath | ImportStep::NewDeckName
                ) {
                    self.input_buffer.push(c);
                }
            }
            Action::Backspace => {
                if matches!(
                    self.import_step,
                    ImportStep::FilePath | ImportStep::NewDeckName
                ) {
                    self.input_buffer.pop();
                }
            }
            Action::Up => self.import_move_selection(-1),
            Action::Down => self.import_move_selection(1),
            Action::Quit => self.should_quit = true,
            _ => {}
        }
        Ok(())
    }

    pub fn process_event(&mut self, event: crossterm::event::KeyEvent) -> Result<()> {
        if self.import_status.is_some() && matches!(self.current_screen(), Screen::DeckList) {
            self.import_status = None;
            self.import_status_at = None;
        }
        if self.deck_view_message.is_some()
            && matches!(self.current_screen(), Screen::DeckView { .. })
        {
            self.deck_view_message = None;
        }
        let action = if matches!(self.current_screen(), Screen::ImportCsv)
            && !matches!(
                self.import_step,
                ImportStep::FilePath | ImportStep::NewDeckName
            ) {
            map_key(event)
        } else if self.is_input_screen() {
            map_key_in_input(event)
        } else {
            map_key(event)
        };
        if let Some(action) = action {
            self.handle_key(action)?;
        }
        Ok(())
    }

    /// Drains any pending auth worker messages. Called once per UI tick after key
    /// processing. Failure modes are converted to AuthError so the user can retry.
    pub fn poll_auth_updates(&mut self) -> Result<()> {
        if self
            .import_status_at
            .is_some_and(|at| at.elapsed() >= Duration::from_secs(2))
        {
            self.import_status = None;
            self.import_status_at = None;
        }

        let Some(rx) = self.auth_rx.as_ref() else {
            return Ok(());
        };
        let update = match rx.try_recv() {
            Ok(u) => u,
            Err(mpsc::TryRecvError::Empty) => return Ok(()),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.auth_rx = None;
                self.auth_cancel = None;
                return Ok(());
            }
        };
        match update {
            AuthUpdate::Completed(session) => self.complete_login(session),
            AuthUpdate::Failed(msg) => self.show_auth_error(msg),
        }
        Ok(())
    }

    fn complete_login(&mut self, session: Session) {
        if let Err(e) = session.save() {
            // Disk-write failure must not crash the app. Surface it on the
            // error screen so the user knows the next launch will re-login.
            self.show_auth_error(format!(
                "login succeeded but could not persist session: {e}"
            ));
            return;
        }
        self.session = Some(session);
        self.screen_stack
            .retain(|s| !matches!(s, Screen::DeviceAuth { .. } | Screen::AuthError { .. }));
        // If Welcome is still on the stack (welcome → device_auth → success path),
        // drop it so we land cleanly on DeckList.
        if matches!(self.screen_stack.last(), Some(Screen::Welcome)) {
            self.screen_stack.pop();
        }
        self.screen_stack.push(Screen::DeckList);
        self.refresh_decks().ok();
        self.auth_rx = None;
        self.auth_cancel = None;
    }

    fn show_auth_error(&mut self, message: String) {
        // Drop any half-active auth state so the error screen isn't bombarded
        // by stale messages from a thread that was already cancelled.
        self.auth_rx = None;
        self.auth_cancel = None;
        self.screen_stack
            .retain(|s| !matches!(s, Screen::DeviceAuth { .. } | Screen::AuthError { .. }));
        self.screen_stack.push(Screen::AuthError { message });
    }

    fn start_device_flow(&mut self) -> Result<()> {
        let client_id = match std::env::var("GITHUB_CLIENT_ID") {
            Ok(v) if !v.is_empty() => v,
            _ => {
                self.show_auth_error(
                    "GITHUB_CLIENT_ID is not set — copy .env.example to .env and configure it"
                        .to_string(),
                );
                return Ok(());
            }
        };

        let resp = match github::request_device_code(&client_id) {
            Ok(r) => r,
            Err(e) => {
                self.show_auth_error(format!("could not reach github: {e}"));
                return Ok(());
            }
        };

        self.screen_stack
            .retain(|s| !matches!(s, Screen::DeviceAuth { .. } | Screen::AuthError { .. }));
        self.screen_stack.push(Screen::DeviceAuth {
            user_code: resp.user_code.clone(),
            verification_uri: resp.verification_uri.clone(),
        });

        let (tx, rx) = mpsc::channel();
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let cancel_for_thread = cancel.clone();
        let device_code = resp.device_code.clone();
        let mut interval = resp.interval;
        thread::spawn(move || {
            let poll =
                github::poll_for_token(&client_id, &device_code, &mut interval, &cancel_for_thread);
            let update = match poll {
                github::PollResult::Success(token) => match github::fetch_user(&token) {
                    Ok(user) => {
                        if let (Ok(url), Ok(key)) = (
                            std::env::var("SUPABASE_URL"),
                            std::env::var("SUPABASE_ANON_KEY"),
                        ) {
                            if let Err(e) = supabase::upsert_user(&url, &key, &user) {
                                let _ = tx.send(AuthUpdate::Failed(format!(
                                    "supabase upsert failed: {e}"
                                )));
                                return;
                            }
                        }
                        AuthUpdate::Completed(Session {
                            github_token: token,
                            github_id: user.id,
                            github_username: user.login,
                            avatar_url: user.avatar_url,
                        })
                    }
                    Err(e) => AuthUpdate::Failed(format!("github /user failed: {e}")),
                },
                github::PollResult::Error(msg) => AuthUpdate::Failed(msg),
                github::PollResult::Cancelled => return,
            };
            let _ = tx.send(update);
        });

        self.auth_rx = Some(rx);
        self.auth_cancel = Some(cancel);
        Ok(())
    }

    fn open_verification_url(&self) {
        let Screen::DeviceAuth {
            verification_uri, ..
        } = self.current_screen().clone()
        else {
            return;
        };
        // Silent on failure: the on-screen hint already tells the user
        // they can visit the URL manually.
        let _ = open::that(&verification_uri);
    }

    fn retry_auth(&mut self) -> Result<()> {
        self.start_device_flow()
    }

    fn handle_auth_back(&mut self) -> Result<()> {
        match self.current_screen() {
            Screen::Welcome => {
                // Welcome is the bottom of the stack — Esc does nothing here.
            }
            Screen::DeviceAuth { .. } | Screen::AuthError { .. } => {
                self.cancel_auth();
                self.screen_stack
                    .retain(|s| !matches!(s, Screen::DeviceAuth { .. } | Screen::AuthError { .. }));
            }
            _ => {}
        }
        Ok(())
    }

    fn cancel_auth(&mut self) {
        if let Some(cancel) = self.auth_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.auth_rx = None;
    }

    fn start_search(&mut self) -> Result<()> {
        if !matches!(self.current_screen(), Screen::DeckList) {
            return Ok(());
        }
        self.search_query.clear();
        self.search_selected = 0;
        self.refresh_search()?;
        self.screen_stack.push(Screen::Search);
        Ok(())
    }

    fn refresh_search(&mut self) -> Result<()> {
        self.search_results = db::search_cards(&self.conn, &self.search_query)?;
        if self.search_results.is_empty() {
            self.search_selected = 0;
        } else if self.search_selected >= self.search_results.len() {
            self.search_selected = self.search_results.len() - 1;
        }
        Ok(())
    }

    fn move_search_selection(&mut self, delta: i32) {
        if !self.search_results.is_empty() {
            let len = self.search_results.len();
            self.search_selected =
                (self.search_selected as i32 + delta).rem_euclid(len as i32) as usize;
        }
    }

    fn open_search_result(&mut self) -> Result<()> {
        let Some((card, _deck_name)) = self.search_results.get(self.search_selected).cloned()
        else {
            return Ok(());
        };
        self.current_deck = db::get_deck(&self.conn, card.deck_id)?;
        self.card_draft_front = card.front.clone();
        self.card_draft_back = card.back.clone();
        self.input_buffer = card.front;
        self.card_modal_step = CardModalStep::Front;
        self.screen_stack.push(Screen::CardModal {
            deck_id: card.deck_id,
            editing: Some(card.id),
        });
        Ok(())
    }

    fn import_move_selection(&mut self, delta: i32) {
        let len = match self.import_step {
            ImportStep::DeckChoice => 2,
            ImportStep::ExistingDeck => self.import_decks.len(),
            _ => 0,
        };
        if len > 0 {
            self.import_selected =
                (self.import_selected as i32 + delta).rem_euclid(len as i32) as usize;
        }
    }

    fn move_selection(&mut self, delta: i32) {
        match self.current_screen() {
            Screen::DeckList if !self.decks.is_empty() => {
                let len = self.decks.len();
                let next = self.deck_list_selected as i32 + delta;
                self.deck_list_selected = next.rem_euclid(len as i32) as usize;
            }
            Screen::DeckView { .. } if !self.cards.is_empty() => {
                let len = self.cards.len();
                let next = self.deck_view_selected as i32 + delta;
                self.deck_view_selected = next.rem_euclid(len as i32) as usize;
            }
            _ => {}
        }
    }

    fn handle_confirm(&mut self) -> Result<()> {
        match self.current_screen().clone() {
            Screen::DeckList => {
                if let Some(summary) = self.decks.get(self.deck_list_selected) {
                    self.open_deck(summary.deck.id)?;
                }
            }
            Screen::DeckView { .. } => self.handle_edit()?,
            _ => {}
        }
        Ok(())
    }

    fn start_import(&mut self) -> Result<()> {
        if !matches!(self.current_screen(), Screen::DeckList) {
            return Ok(());
        }
        self.input_buffer.clear();
        self.import_error = None;
        self.import_status = None;
        self.import_status_at = None;
        self.import_preview = None;
        self.import_decks.clear();
        self.import_selected = 0;
        self.import_deck_name.clear();
        self.import_path = None;
        self.import_step = ImportStep::FilePath;
        self.screen_stack.push(Screen::ImportCsv);
        Ok(())
    }

    fn import_confirm(&mut self) -> Result<()> {
        match self.import_step {
            ImportStep::FilePath => {
                let raw_path = self.input_buffer.trim();
                let path = expand_path(raw_path);
                if !path.is_file() {
                    self.import_error = Some("file not found — try again".to_string());
                    return Ok(());
                }
                match CsvPreview::from_path(&path) {
                    Ok(preview) => {
                        self.import_path = Some(path);
                        self.import_preview = Some(preview);
                        self.import_error = None;
                        self.input_buffer.clear();
                        self.import_step = ImportStep::Preview;
                    }
                    Err(error) => {
                        let message = error.to_string();
                        self.import_error = Some(if message.contains("csv must have") {
                            "csv must have 'front' and 'back' columns".to_string()
                        } else {
                            message
                        });
                    }
                }
            }
            ImportStep::Preview => {
                self.import_decks = db::list_deck_names(&self.conn)?;
                self.import_selected = 0;
                self.import_step = ImportStep::DeckChoice;
            }
            ImportStep::DeckChoice => {
                if self.import_selected == 0 || self.import_decks.is_empty() {
                    self.input_buffer.clear();
                    self.import_error = None;
                    self.import_step = ImportStep::NewDeckName;
                } else {
                    self.import_selected = 0;
                    self.import_step = ImportStep::ExistingDeck;
                }
            }
            ImportStep::NewDeckName => {
                let name = self.input_buffer.trim();
                if name.is_empty() {
                    self.import_error = Some("deck name cannot be empty".to_string());
                } else if db::deck_name_exists(&self.conn, name)? {
                    self.import_error = Some("deck name already exists — try again".to_string());
                } else {
                    self.import_deck_name = name.to_string();
                    self.import_error = None;
                    self.import_step = ImportStep::Confirm;
                }
            }
            ImportStep::ExistingDeck => {
                if let Some((_, name)) = self.import_decks.get(self.import_selected) {
                    self.import_deck_name = name.clone();
                    self.import_step = ImportStep::Confirm;
                }
            }
            ImportStep::Confirm => {
                let path = self.import_path.as_ref().expect("path before confirm");
                let final_preview = match CsvPreview::from_path(path) {
                    Ok(preview) => preview,
                    Err(error) => {
                        self.import_error = Some(format!("could not reread csv file: {error}"));
                        return Ok(());
                    }
                };
                self.import_preview = Some(final_preview.clone());
                let (imported, skipped) = if let Some((deck_id, _)) = self
                    .import_decks
                    .iter()
                    .find(|(_, name)| name == &self.import_deck_name)
                {
                    (
                        db::import_cards(&mut self.conn, *deck_id, &final_preview.cards)?,
                        final_preview.skipped_rows,
                    )
                } else {
                    let (deck_id, imported) = db::create_deck_and_import_cards(
                        &mut self.conn,
                        &self.import_deck_name,
                        &final_preview.cards,
                    )?;
                    let _ = deck_id;
                    (imported, final_preview.skipped_rows)
                };
                let message = if skipped == 0 {
                    format!("imported {imported} cards into '{}'", self.import_deck_name)
                } else {
                    format!(
                        "imported {imported} cards into '{}' ({skipped} rows skipped — missing front or back)",
                        self.import_deck_name
                    )
                };
                self.screen_stack.pop();
                self.input_buffer.clear();
                self.import_preview = None;
                self.import_error = None;
                self.import_status = Some(message);
                self.import_status_at = Some(Instant::now());
                self.refresh_decks()?;
            }
        }
        Ok(())
    }

    fn import_back(&mut self) -> Result<()> {
        match self.import_step {
            ImportStep::FilePath => self.pop_screen()?,
            ImportStep::Preview => {
                self.import_step = ImportStep::FilePath;
                self.input_buffer = self
                    .import_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default();
                self.import_error = None;
            }
            ImportStep::DeckChoice => {
                self.import_step = ImportStep::Preview;
                self.import_selected = 0;
            }
            ImportStep::NewDeckName => {
                self.import_step = ImportStep::DeckChoice;
                self.import_selected = 0;
                self.input_buffer.clear();
                self.import_error = None;
            }
            ImportStep::ExistingDeck => {
                self.import_step = ImportStep::DeckChoice;
                self.import_selected = 1;
            }
            ImportStep::Confirm => {
                self.import_step = ImportStep::DeckChoice;
                self.import_selected = 0;
                self.import_deck_name.clear();
            }
        }
        Ok(())
    }

    fn handle_new(&mut self) -> Result<()> {
        match self.current_screen().clone() {
            Screen::DeckList => {
                self.input_buffer.clear();
                self.screen_stack.push(Screen::NewDeckModal);
            }
            Screen::DeckView { deck_id } => {
                self.input_buffer.clear();
                self.card_modal_step = CardModalStep::Front;
                self.card_draft_front.clear();
                self.card_draft_back.clear();
                self.screen_stack.push(Screen::CardModal {
                    deck_id,
                    editing: None,
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_edit(&mut self) -> Result<()> {
        if let Screen::DeckView { deck_id } = self.current_screen().clone() {
            if let Some(entry) = self.cards.get(self.deck_view_selected) {
                self.card_draft_front = entry.card.front.clone();
                self.card_draft_back = entry.card.back.clone();
                self.input_buffer = entry.card.front.clone();
                self.card_modal_step = CardModalStep::Front;
                self.screen_stack.push(Screen::CardModal {
                    deck_id,
                    editing: Some(entry.card.id),
                });
            }
        }
        Ok(())
    }

    fn handle_rename(&mut self) -> Result<()> {
        if let Screen::DeckList = self.current_screen().clone() {
            if let Some(summary) = self.decks.get(self.deck_list_selected) {
                self.input_buffer = summary.deck.name.clone();
                self.screen_stack.push(Screen::RenameDeckModal {
                    deck_id: summary.deck.id,
                });
            }
        }
        Ok(())
    }

    fn handle_delete(&mut self) -> Result<()> {
        let now = Instant::now();
        let confirm_window = Duration::from_secs(2);

        match self.current_screen().clone() {
            Screen::DeckList => {
                if let Some(summary) = self.decks.get(self.deck_list_selected) {
                    let target = DeleteTarget::Deck(summary.deck.id);
                    if self.delete_pending == Some(target)
                        && self
                            .delete_pending_at
                            .is_some_and(|t| now.duration_since(t) <= confirm_window)
                    {
                        db::delete_deck(&self.conn, summary.deck.id)?;
                        self.clear_delete_pending();
                        self.refresh_decks()?;
                        if self.deck_list_selected >= self.decks.len() && !self.decks.is_empty() {
                            self.deck_list_selected = self.decks.len() - 1;
                        }
                    } else {
                        self.delete_pending = Some(target);
                        self.delete_pending_at = Some(now);
                    }
                }
            }
            Screen::DeckView { deck_id } => {
                if let Some(entry) = self.cards.get(self.deck_view_selected) {
                    let target = DeleteTarget::Card(entry.card.id);
                    if self.delete_pending == Some(target)
                        && self
                            .delete_pending_at
                            .is_some_and(|t| now.duration_since(t) <= confirm_window)
                    {
                        db::delete_card(&self.conn, entry.card.id)?;
                        self.clear_delete_pending();
                        self.refresh_deck_view(deck_id)?;
                        if self.deck_view_selected >= self.cards.len() && !self.cards.is_empty() {
                            self.deck_view_selected = self.cards.len() - 1;
                        }
                        self.refresh_decks()?;
                    } else {
                        self.delete_pending = Some(target);
                        self.delete_pending_at = Some(now);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_stats(&mut self) -> Result<()> {
        if !matches!(self.current_screen(), Screen::DeckList) {
            return Ok(());
        }
        let retention = db::retention_rate_30d(&self.conn)?;
        let heatmap = db::review_heatmap_90d(&self.conn)?;
        let forecast = db::forecast_14d(&self.conn)?;
        self.stats_state = Some(StatsState {
            retention,
            heatmap,
            forecast,
            view: HeatmapView::Daily,
        });
        self.screen_stack.push(Screen::Stats);
        Ok(())
    }

    fn handle_toggle_view(&mut self) {
        if matches!(self.current_screen(), Screen::Stats) {
            if let Some(stats) = self.stats_state.as_mut() {
                stats.view = match stats.view {
                    HeatmapView::Daily => HeatmapView::Weekly,
                    HeatmapView::Weekly => HeatmapView::Daily,
                };
            }
        }
    }

    fn handle_start_review(&mut self, cram: bool) -> Result<()> {
        if let Screen::DeckView { deck_id } = self.current_screen().clone() {
            self.deck_view_message = None;
            let mut queue = if cram {
                db::list_cards(&self.conn, deck_id)?
            } else {
                db::get_due_cards(&self.conn, deck_id, Utc::now())?
            };

            if cram {
                if queue.is_empty() {
                    self.deck_view_message = Some("no cards in this deck yet".to_string());
                    return Ok(());
                }
                shuffle_cards(&mut queue);
            }

            self.review_queue = queue;
            self.review_index = 0;
            self.review_flipped = false;
            self.review_message = None;
            self.screen_stack.push(Screen::Review { deck_id, cram });
        }
        Ok(())
    }

    fn handle_flip(&mut self) {
        if matches!(self.current_screen(), Screen::Review { .. }) && self.review_current().is_some()
        {
            self.review_flipped = true;
        }
    }

    fn handle_rate(&mut self, rating: u8) -> Result<()> {
        let Screen::Review { deck_id, cram } = self.current_screen().clone() else {
            return Ok(());
        };
        if !self.review_flipped {
            return Ok(());
        }
        let Some(entry) = self.review_queue.get(self.review_index).cloned() else {
            return Ok(());
        };

        let fsrs_rating = match rating {
            1 => Rating::Again,
            2 => Rating::Hard,
            3 => Rating::Good,
            4 => Rating::Easy,
            _ => return Ok(()),
        };

        persist_review_rating(
            &self.conn,
            entry.review.as_ref(),
            entry.card.id,
            fsrs_rating,
            rating,
            cram,
        )?;

        self.review_index += 1;
        self.review_flipped = false;

        if self.review_index >= self.review_queue.len() {
            if cram {
                self.deck_view_message = Some(format!(
                    "cram session complete — {} cards reviewed",
                    self.review_queue.len()
                ));
            } else {
                self.refresh_deck_view(deck_id)?;
                self.refresh_decks()?;
            }
            self.screen_stack.pop();
            self.review_flipped = false;
            self.review_message = None;
        }

        Ok(())
    }

    fn submit_input(&mut self) -> Result<()> {
        match self.current_screen().clone() {
            Screen::NewDeckModal => {
                let name = self.input_buffer.trim();
                if !name.is_empty() {
                    db::create_deck(&self.conn, name)?;
                    self.pop_screen()?;
                    self.refresh_decks()?;
                }
            }
            Screen::RenameDeckModal { deck_id } => {
                let name = self.input_buffer.trim();
                if !name.is_empty() {
                    db::rename_deck(&self.conn, deck_id, name)?;
                    self.pop_screen()?;
                    self.refresh_decks()?;
                    if self.current_deck.as_ref().is_some_and(|d| d.id == deck_id) {
                        self.current_deck = db::get_deck(&self.conn, deck_id)?;
                    }
                }
            }
            Screen::CardModal { deck_id, editing } => match self.card_modal_step {
                CardModalStep::Front => {
                    self.card_draft_front = self.input_buffer.trim().to_string();
                    if self.card_draft_front.is_empty() {
                        return Ok(());
                    }
                    self.card_modal_step = CardModalStep::Back;
                    self.input_buffer = if editing.is_some() {
                        self.card_draft_back.clone()
                    } else {
                        String::new()
                    };
                }
                CardModalStep::Back => {
                    self.card_draft_back = self.input_buffer.trim().to_string();
                    if self.card_draft_back.is_empty() {
                        return Ok(());
                    }
                    self.card_modal_step = CardModalStep::Tags;
                    self.input_buffer = if let Some(card_id) = editing {
                        db::get_card(&self.conn, card_id)?
                            .and_then(|c| c.tags)
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                }
                CardModalStep::Tags => {
                    let tags = self.input_buffer.trim();
                    let tags_opt = if tags.is_empty() { None } else { Some(tags) };
                    if let Some(card_id) = editing {
                        db::update_card(
                            &self.conn,
                            card_id,
                            &self.card_draft_front,
                            &self.card_draft_back,
                            tags_opt,
                        )?;
                    } else {
                        db::create_card(
                            &self.conn,
                            deck_id,
                            &self.card_draft_front,
                            &self.card_draft_back,
                            tags_opt,
                        )?;
                    }
                    self.pop_screen()?;
                    self.refresh_deck_view(deck_id)?;
                    self.refresh_decks()?;
                }
            },
            _ => {}
        }
        Ok(())
    }

    fn open_deck(&mut self, deck_id: i64) -> Result<()> {
        self.refresh_deck_view(deck_id)?;
        self.deck_view_selected = 0;
        self.clear_delete_pending();
        self.screen_stack.push(Screen::DeckView { deck_id });
        Ok(())
    }

    fn pop_screen(&mut self) -> Result<()> {
        if self.screen_stack.len() > 1 {
            self.screen_stack.pop();
            self.input_buffer.clear();
            self.clear_delete_pending();

            match self.current_screen() {
                Screen::DeckList => self.refresh_decks()?,
                Screen::Search => self.refresh_search()?,
                _ => {}
            }
        }
        Ok(())
    }

    fn clear_delete_pending(&mut self) {
        self.delete_pending = None;
        self.delete_pending_at = None;
    }

    fn refresh_decks(&mut self) -> Result<()> {
        self.decks = db::list_decks(&self.conn, Utc::now())?;
        Ok(())
    }

    fn refresh_deck_view(&mut self, deck_id: i64) -> Result<()> {
        self.current_deck = db::get_deck(&self.conn, deck_id)?;
        self.cards = db::list_cards(&self.conn, deck_id)?;
        Ok(())
    }
}

fn persist_review_rating(
    conn: &Connection,
    review: Option<&db::ReviewState>,
    card_id: i64,
    fsrs_rating: Rating,
    rating: u8,
    cram: bool,
) -> Result<()> {
    if cram {
        return Ok(());
    }

    let now = Utc::now();
    let (new_state, elapsed_days) = schedule(review, card_id, fsrs_rating, now);
    db::upsert_review_state(conn, &new_state)?;
    db::insert_review_log(conn, card_id, rating as i32, now, elapsed_days)?;
    Ok(())
}

fn shuffle_cards(cards: &mut [CardWithReview]) {
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);

    for index in (1..cards.len()).rev() {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let swap_index = (seed % (index as u64 + 1)) as usize;
        cards.swap(index, swap_index);
    }
}

fn expand_path(raw: &str) -> PathBuf {
    if raw == "~" {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(raw))
    } else if let Some(rest) = raw.strip_prefix("~/") {
        dirs::home_dir()
            .map(|home| home.join(rest))
            .unwrap_or_else(|| PathBuf::from(raw))
    } else {
        PathBuf::from(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE review_state (
                card_id INTEGER PRIMARY KEY,
                stability REAL NOT NULL DEFAULT 0,
                difficulty REAL NOT NULL DEFAULT 0,
                due_date TEXT NOT NULL,
                last_review TEXT,
                reps INTEGER NOT NULL DEFAULT 0,
                lapses INTEGER NOT NULL DEFAULT 0,
                state TEXT NOT NULL DEFAULT 'new'
            );
            CREATE TABLE review_log (
                id INTEGER PRIMARY KEY,
                card_id INTEGER NOT NULL,
                rating INTEGER NOT NULL,
                reviewed_at TEXT NOT NULL,
                elapsed_days REAL NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn cram_rating_does_not_write_review_state_or_log() {
        let conn = review_conn();

        persist_review_rating(&conn, None, 1, Rating::Good, 3, true).unwrap();

        let state_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM review_state", [], |row| row.get(0))
            .unwrap();
        let log_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM review_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(state_count, 0);
        assert_eq!(log_count, 0);
    }

    #[test]
    fn regular_rating_writes_review_state_and_log() {
        let conn = review_conn();

        persist_review_rating(&conn, None, 1, Rating::Good, 3, false).unwrap();

        let state_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM review_state", [], |row| row.get(0))
            .unwrap();
        let log_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM review_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(state_count, 1);
        assert_eq!(log_count, 1);

        let persisted_card_id: i64 = conn
            .query_row("SELECT card_id FROM review_state", [], |row| row.get(0))
            .unwrap();
        let persisted_rating: i64 = conn
            .query_row("SELECT rating FROM review_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(persisted_card_id, 1);
        assert_eq!(persisted_rating, 3);
    }
}
