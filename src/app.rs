use crate::auth::{AuthUpdate, Session, github, supabase};
use crate::db::{self, CardWithReview, Deck, DeckSummary};
use crate::events::{Action, map_key, map_key_in_input};
use crate::fsrs::scheduler::schedule;
use crate::ui::CardModalStep;
use anyhow::Result;
use chrono::{NaiveDate, Utc};
use rs_fsrs::Rating;
use rusqlite::Connection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

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
            Action::Review => self.handle_start_review()?,
            Action::Stats => self.handle_stats()?,
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
            Screen::CardModal { .. } | Screen::NewDeckModal | Screen::RenameDeckModal { .. }
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

    pub fn process_event(&mut self, event: crossterm::event::KeyEvent) -> Result<()> {
        let action = if self.is_input_screen() {
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

    fn handle_start_review(&mut self) -> Result<()> {
        if let Screen::DeckView { deck_id } = self.current_screen().clone() {
            let now = Utc::now();
            self.review_queue = db::get_due_cards(&self.conn, deck_id, now)?;
            self.review_index = 0;
            self.review_flipped = false;
            self.review_message = None;
            self.screen_stack.push(Screen::Review { deck_id });
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
        let Screen::Review { deck_id } = self.current_screen().clone() else {
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

        let now = Utc::now();
        let (new_state, elapsed_days) =
            schedule(entry.review.as_ref(), entry.card.id, fsrs_rating, now);
        db::upsert_review_state(&self.conn, &new_state)?;
        db::insert_review_log(&self.conn, entry.card.id, rating as i32, now, elapsed_days)?;

        self.review_index += 1;
        self.review_flipped = false;

        if self.review_index >= self.review_queue.len() {
            self.refresh_deck_view(deck_id)?;
            self.refresh_decks()?;
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

            if matches!(self.current_screen(), Screen::DeckList) {
                self.refresh_decks()?;
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
