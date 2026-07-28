use crate::db::{
    self, CardWithReview, Deck, DeckSummary,
};
use crate::events::{map_key, map_key_in_input, Action};
use crate::fsrs::scheduler::schedule;
use crate::ui::CardModalStep;
use anyhow::Result;
use chrono::Utc;
use rs_fsrs::Rating;
use rusqlite::Connection;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    DeckList,
    DeckView { deck_id: i64 },
    Review { deck_id: i64 },
    CardModal {
        deck_id: i64,
        editing: Option<i64>,
    },
    NewDeckModal,
    RenameDeckModal { deck_id: i64 },
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
}

impl App {
    pub fn new(conn: Connection) -> Result<Self> {
        let mut app = Self {
            conn,
            should_quit: false,
            screen_stack: vec![Screen::DeckList],
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
        };
        app.refresh_decks()?;
        Ok(app)
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
        db::insert_review_log(
            &self.conn,
            entry.card.id,
            rating as i32,
            now,
            elapsed_days,
        )?;

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
