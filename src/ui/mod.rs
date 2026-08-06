use crate::app::{App, Screen};
use ratatui::Frame;

mod auth_error;
mod card_modal;
mod deck_list;
mod deck_view;
mod device_auth;
mod import_csv;
mod review;
mod search;
mod stats;
pub mod theme;
mod welcome;

pub use card_modal::CardModalStep;

pub fn draw(f: &mut Frame, app: &App) {
    match app.current_screen() {
        Screen::Welcome => welcome::draw(f),
        Screen::DeviceAuth {
            user_code,
            verification_uri,
        } => device_auth::draw(f, user_code, verification_uri),
        Screen::AuthError { message } => auth_error::draw(f, message),
        Screen::DeckList => deck_list::draw(
            f,
            &app.decks,
            app.deck_list_selected,
            &deck_list::deck_list_hint(app.delete_pending()),
            app.import_status.as_deref(),
        ),
        Screen::DeckView { .. } => {
            if let Some(deck) = &app.current_deck {
                deck_view::draw(
                    f,
                    deck,
                    &app.cards,
                    app.deck_view_selected,
                    &deck_view::deck_view_hint(app.delete_pending()),
                );
            }
        }
        Screen::Review { .. } => review::draw(
            f,
            app.review_current(),
            app.review_flipped,
            app.review_queue.len().saturating_sub(app.review_index),
            app.review_message.as_deref(),
        ),
        Screen::CardModal { editing, .. } => card_modal::draw(
            f,
            app.current_deck
                .as_ref()
                .map(|d| d.name.as_str())
                .unwrap_or(""),
            app.card_modal_step,
            &app.input_buffer,
            editing.is_some(),
        ),
        Screen::NewDeckModal => {
            card_modal::draw_simple_modal(f, "New Deck", "Deck name", &app.input_buffer);
        }
        Screen::RenameDeckModal { .. } => {
            card_modal::draw_simple_modal(f, "Rename Deck", "New name", &app.input_buffer);
        }
        Screen::Stats => stats::draw(f, app),
        Screen::Search => search::draw(
            f,
            &app.search_query,
            &app.search_results,
            app.search_selected,
        ),
        Screen::ImportCsv => import_csv::draw(
            f,
            app.import_step,
            &app.input_buffer,
            app.import_error.as_deref(),
            app.import_preview.as_ref(),
            &app.import_decks,
            app.import_selected,
            &app.import_deck_name,
        ),
    }
}
