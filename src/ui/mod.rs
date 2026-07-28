use ratatui::Frame;

use crate::app::App;

mod card_modal;
mod deck_list;
mod deck_view;
mod review;
pub mod theme;

pub use card_modal::CardModalStep;

pub fn draw(f: &mut Frame, app: &App) {
    match app.current_screen() {
        crate::app::Screen::DeckList => deck_list::draw(
            f,
            &app.decks,
            app.deck_list_selected,
            &deck_list::deck_list_hint(app.delete_pending()),
        ),
        crate::app::Screen::DeckView { .. } => {
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
        crate::app::Screen::Review { .. } => review::draw(
            f,
            app.review_current(),
            app.review_flipped,
            app.review_queue.len().saturating_sub(app.review_index),
            app.review_message.as_deref(),
        ),
        crate::app::Screen::CardModal { editing, .. } => card_modal::draw(
            f,
            app.current_deck
                .as_ref()
                .map(|d| d.name.as_str())
                .unwrap_or(""),
            app.card_modal_step,
            &app.input_buffer,
            editing.is_some(),
        ),
        crate::app::Screen::NewDeckModal => {
            card_modal::draw_simple_modal(f, "New Deck", "Deck name", &app.input_buffer);
        }
        crate::app::Screen::RenameDeckModal { .. } => {
            card_modal::draw_simple_modal(f, "Rename Deck", "New name", &app.input_buffer);
        }
    }
}
