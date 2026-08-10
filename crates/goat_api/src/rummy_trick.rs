use std::fmt;
use std::fmt::Debug;

use smallvec::SmallVec;

use crate::{Card, Cards, PlayerIdx, Suit};

const CARD_MASK: u16 = 0x3f;
const HI_SHIFT: u32 = 6;
const PLAYER_SHIFT: u32 = 12;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct RummyPlay(u16);

impl RummyPlay {
    pub fn new(player: PlayerIdx, lo: Card, hi: Card) -> Self {
        assert!(player.0 < 16, "rummy only supports up to 16 players");
        Self((lo as u16) | ((hi as u16) << HI_SHIFT) | ((player.0 as u16) << PLAYER_SHIFT))
    }

    pub fn player(self) -> PlayerIdx {
        PlayerIdx((self.0 >> PLAYER_SHIFT) as u8)
    }

    pub fn lo(self) -> Card {
        Card::from((self.0 & CARD_MASK) as u8)
    }

    pub fn hi(self) -> Card {
        Card::from(((self.0 >> HI_SHIFT) & CARD_MASK) as u8)
    }
}

impl Debug for RummyPlay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RummyPlay")
            .field("player", &self.player())
            .field("cards", &Cards::range(self.lo(), self.hi()))
            .finish()
    }
}

#[derive(Clone)]
pub struct RummyTrick {
    plays: SmallVec<[RummyPlay; 12]>,
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

    pub fn plays(&self) -> &[RummyPlay] {
        &self.plays
    }

    pub fn top_card(&self) -> Option<Card> {
        Some(self.plays.last()?.hi())
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
        let first = self.plays[0];
        let mut range = (first.lo(), first.hi());
        let mut shift = 1;
        for play in &self.plays[1..] {
            let lo = play.lo();
            if lo.suit() == range.1.suit() && lo.rank().idx() == range.1.rank().idx() + 1 {
                range.1 = play.hi();
                shift += 1;
            } else {
                break;
            }
        }
        self.plays.drain(..shift);
        range
    }

    pub fn play(&mut self, player: PlayerIdx, lo: Card, hi: Card) -> bool {
        self.plays.push(RummyPlay::new(player, lo, hi));
        self.plays.len() == self.num_players
    }
}

impl Debug for RummyTrick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("");
        f.field(&self.num_players);
        for play in &self.plays {
            f.field(play);
        }
        f.finish()
    }
}
