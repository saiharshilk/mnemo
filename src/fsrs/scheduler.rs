use crate::db::{CardState, ReviewState};
use chrono::{DateTime, Duration, Utc};
use rs_fsrs::{Card as FsrsCard, FSRS, Rating, State as FsrsState};

fn to_fsrs_state(state: CardState) -> FsrsState {
    match state {
        CardState::New => FsrsState::New,
        CardState::Learning => FsrsState::Learning,
        CardState::Review => FsrsState::Review,
        CardState::Relearning => FsrsState::Relearning,
    }
}

fn from_fsrs_state(state: FsrsState) -> CardState {
    match state {
        FsrsState::New => CardState::New,
        FsrsState::Learning => CardState::Learning,
        FsrsState::Review => CardState::Review,
        FsrsState::Relearning => CardState::Relearning,
    }
}

pub fn to_fsrs_card(review: Option<&ReviewState>, now: DateTime<Utc>) -> FsrsCard {
    match review {
        None => FsrsCard::new(),
        Some(rs) => FsrsCard {
            due: rs.due_date,
            stability: rs.stability,
            difficulty: rs.difficulty,
            elapsed_days: 0,
            scheduled_days: 0,
            reps: rs.reps as i32,
            lapses: rs.lapses as i32,
            state: to_fsrs_state(rs.state),
            last_review: rs.last_review.unwrap_or(now),
        },
    }
}

pub fn from_fsrs_card(card_id: i64, card: &FsrsCard) -> ReviewState {
    ReviewState {
        card_id,
        stability: card.stability,
        difficulty: card.difficulty,
        due_date: card.due,
        last_review: if card.reps > 0 {
            Some(card.last_review)
        } else {
            None
        },
        reps: card.reps as i64,
        lapses: card.lapses as i64,
        state: from_fsrs_state(card.state),
    }
}

pub fn schedule(
    review: Option<&ReviewState>,
    card_id: i64,
    rating: Rating,
    now: DateTime<Utc>,
) -> (ReviewState, f64) {
    let fsrs = FSRS::default();
    let fsrs_card = to_fsrs_card(review, now);
    let result = fsrs.next(fsrs_card, now, rating);
    let elapsed_days = result.review_log.elapsed_days as f64;
    (from_fsrs_card(card_id, &result.card), elapsed_days)
}

pub fn preview_intervals(review: Option<&ReviewState>, now: DateTime<Utc>) -> [Duration; 4] {
    let fsrs = FSRS::default();
    let fsrs_card = to_fsrs_card(review, now);
    let record_log = fsrs.repeat(fsrs_card, now);
    let mut intervals = [Duration::zero(); 4];
    for rating in Rating::iter() {
        let info = &record_log[rating];
        let days = info.card.scheduled_days.max(0) as i64;
        let idx = match rating {
            Rating::Again => 0,
            Rating::Hard => 1,
            Rating::Good => 2,
            Rating::Easy => 3,
        };
        intervals[idx] = Duration::days(days);
    }
    intervals
}

pub fn format_interval(duration: Duration) -> String {
    let days = duration.num_days();
    if days <= 0 {
        let minutes = duration.num_minutes().max(1);
        if minutes < 60 {
            format!("{minutes}m")
        } else {
            format!("{}h", minutes / 60)
        }
    } else if days == 1 {
        "1d".to_string()
    } else {
        format!("{days}d")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap()
    }

    #[test]
    fn again_resets_or_shortens_interval() {
        let now = fixed_now();
        let mut card = FsrsCard::new();

        for _ in 0..3 {
            let result = FSRS::default().next(card, now, Rating::Good);
            card = result.card;
        }

        let mature = from_fsrs_card(1, &card);
        assert_eq!(mature.state, CardState::Review);
        assert!(mature.stability > 0.0);

        let (after_again, _) = schedule(Some(&mature), 1, Rating::Again, now);
        assert!(
            after_again.due_date <= now + Duration::days(1),
            "again should schedule soon"
        );
    }

    #[test]
    fn easy_grows_interval() {
        let now = fixed_now();
        let (after_good, _) = schedule(None, 1, Rating::Good, now);
        let good_interval = after_good.due_date - now;

        let (after_easy, _) = schedule(None, 2, Rating::Easy, now);
        let easy_interval = after_easy.due_date - now;

        assert!(
            easy_interval >= good_interval,
            "easy interval should be at least as long as good"
        );
    }

    #[test]
    fn preview_intervals_returns_four_entries() {
        let now = fixed_now();
        let intervals = preview_intervals(None, now);
        assert_eq!(intervals.len(), 4);
        assert!(intervals.iter().all(|d| *d >= Duration::zero()));
    }
}
