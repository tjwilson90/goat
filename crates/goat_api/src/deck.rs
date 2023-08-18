use std::fmt;
use std::fmt::Debug;

use crate::Card;

pub trait Deck: Debug {
    fn cards_remaining(&self) -> usize;
}

impl Deck for Vec<Card> {
    fn cards_remaining(&self) -> usize {
        self.len() - 1
    }
}

#[derive(Clone)]
pub struct ClientDeck(u8);

impl ClientDeck {
    pub fn new(num_decks: u8) -> Self {
        Self(52 * num_decks - 1)
    }

    pub fn draw(&mut self) {
        self.0 -= 1;
    }
}

impl Deck for ClientDeck {
    fn cards_remaining(&self) -> usize {
        self.0 as usize
    }
}

impl Debug for ClientDeck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}
