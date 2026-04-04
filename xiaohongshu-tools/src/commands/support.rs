use crate::extract::note::NoteCard;
use std::collections::HashSet;
use std::time::Instant;
use tracing::debug;

pub struct StopCondition {
    max_items: usize,
    duration_secs: u64,
}

impl StopCondition {
    pub fn new(max_items: usize, duration_secs: u64) -> Self {
        Self {
            max_items,
            duration_secs,
        }
    }

    pub fn check(&self, count: usize, start: Instant) -> bool {
        if count >= self.max_items {
            debug!("reached max items limit ({})", self.max_items);
            return true;
        }
        if start.elapsed().as_secs() >= self.duration_secs {
            debug!("reached duration limit ({}s)", self.duration_secs);
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagnationAction {
    Continue,
    Escalate,
    Sprint,
    GiveUp,
}

pub struct StagnationTracker {
    stagnant: u32,
    escalate_at: Option<u32>,
    sprint_at: u32,
    give_up_after_sprint: bool,
    has_sprinted: bool,
}

impl StagnationTracker {
    pub fn for_feed() -> Self {
        Self {
            stagnant: 0,
            escalate_at: None,
            sprint_at: 5,
            give_up_after_sprint: true,
            has_sprinted: false,
        }
    }

    pub fn for_comments() -> Self {
        Self {
            stagnant: 0,
            escalate_at: Some(5),
            sprint_at: 20,
            give_up_after_sprint: false,
            has_sprinted: false,
        }
    }

    pub fn record(&mut self, made_progress: bool) -> StagnationAction {
        if made_progress {
            self.stagnant = 0;
            return StagnationAction::Continue;
        }

        self.stagnant += 1;

        if let Some(esc_at) = self.escalate_at {
            if self.stagnant >= self.sprint_at {
                return StagnationAction::Sprint;
            }
            if self.stagnant >= esc_at {
                return StagnationAction::Escalate;
            }
            return StagnationAction::Continue;
        }

        if self.stagnant >= self.sprint_at {
            if self.give_up_after_sprint && self.has_sprinted {
                return StagnationAction::GiveUp;
            }
            return StagnationAction::Sprint;
        }

        StagnationAction::Continue
    }

    pub fn confirm_sprint(&mut self) {
        self.stagnant = 0;
        self.has_sprinted = true;
    }

    pub fn reset(&mut self) {
        self.stagnant = 0;
    }
}

pub fn card_unique_key(card: &NoteCard) -> String {
    if let Some(id) = &card.id {
        return format!("id:{id}");
    }
    format!("title:{}", card.title)
}

pub fn process_card_batch(
    batch: Vec<NoteCard>,
    seen: &mut HashSet<String>,
    include_kw: &[String],
    exclude_kw: &[String],
    cards: &mut Vec<NoteCard>,
) -> Vec<NoteCard> {
    let mut new_cards = Vec::new();
    for card in batch {
        let key = card_unique_key(&card);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);

        if matches_keywords(&card.title, exclude_kw) {
            continue;
        }
        if !include_kw.is_empty() && !matches_keywords(&card.title, include_kw) {
            continue;
        }

        let id = card.id.clone().unwrap_or_default();
        debug!("[{}] {} | {}", cards.len() + 1, card.title, id);
        new_cards.push(card.clone());
        cards.push(card);
    }
    new_cards
}

pub fn matches_keywords(text: &str, keywords: &[String]) -> bool {
    if keywords.is_empty() {
        return false;
    }
    let lower = text.to_lowercase();
    keywords.iter().any(|kw| lower.contains(&kw.to_lowercase()))
}
