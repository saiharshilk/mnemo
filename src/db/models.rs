use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Deck {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DeckSummary {
    pub deck: Deck,
    pub due_count: i64,
    pub card_count: i64,
}

#[derive(Debug, Clone)]
pub struct Card {
    pub id: i64,
    pub deck_id: i64,
    pub front: String,
    pub back: String,
    pub tags: Option<String>,
    pub note_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CardWithReview {
    pub card: Card,
    pub review: Option<ReviewState>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewState {
    pub card_id: i64,
    pub stability: f64,
    pub difficulty: f64,
    pub due_date: DateTime<Utc>,
    pub last_review: Option<DateTime<Utc>>,
    pub reps: i64,
    pub lapses: i64,
    pub state: CardState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardState {
    New,
    Learning,
    Review,
    Relearning,
}

impl CardState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Learning => "learning",
            Self::Review => "review",
            Self::Relearning => "relearning",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "learning" => Self::Learning,
            "review" => Self::Review,
            "relearning" => Self::Relearning,
            _ => Self::New,
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::New => "◆",
            Self::Learning => "◐",
            Self::Review => "●",
            Self::Relearning => "○",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Learning => "learning",
            Self::Review => "mature",
            Self::Relearning => "lapsed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReviewLogEntry {
    pub id: i64,
    pub card_id: i64,
    pub rating: i32,
    pub reviewed_at: DateTime<Utc>,
    pub elapsed_days: f64,
}
