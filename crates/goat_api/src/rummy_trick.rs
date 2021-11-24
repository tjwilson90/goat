use std::fmt;
use std::fmt::Debug;

use smallvec::SmallVec;

use crate::{Card, Cards, Suit};

#[derive(Clone)]
pub struct RummyTrick {
    plays: SmallVec<[(Card, Card); 12]>,
    num_players: usize,
}

impl RummyTrick {
    pub fn new(num_players: usize) -> Self {
        Self {
            plays: SmallVec::new(),
            num_players,
        }
    }

    pub fn len(&self) -> usize {
        self.plays.len()
    }

    pub fn plays(&self) -> &[(Card, Card)] {
        &*self.plays
    }

    pub fn top_card(&self) -> Option<Card> {
        let (_, top) = self.plays.last()?;
        Some(*top)
    }

    pub fn can_play(&self, card: Card, trump: Suit) -> bool {
        self.top_card().map_or(true, |c| {
            (card.suit() == c.suit() && card.rank() > c.rank())
                || (card.suit() == trump && c.suit() != trump)
        })
    }

    pub fn is_empty(&self) -> bool {
        self.plays.is_empty()
    }

    pub fn num_players(&self) -> usize {
        self.num_players
    }

    pub fn pick_up(&mut self) -> (Card, Card) {
        let mut range = self.plays[0];
        let mut shift = 1;
        for &(lo, hi) in &self.plays[1..] {
            if lo.suit() == range.1.suit() && lo.rank().idx() - range.1.rank().idx() == 1 {
                range.1 = hi;
                shift += 1;
            } else {
                break;
            }
        }
        self.plays.drain(..shift);
        range
    }

    pub fn play(&mut self, lo: Card, hi: Card) -> bool {
        self.plays.push((lo, hi));
        self.plays.len() == self.num_players
    }
}

impl Debug for RummyTrick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("");
        f.field(&self.num_players);
        for (lo, hi) in &self.plays {
            f.field(&Cards::range(*lo, *hi));
        }
        f.finish()
    }
}
