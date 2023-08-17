use std::fmt;
use std::fmt::Debug;

use crate::Card;

pub trait Deck {
    fn is_empty(&self) -> bool;
}

impl Deck for Vec<Card> {
    fn is_empty(&self) -> bool {
        self.len() == 1
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

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn len(&self) -> u8 {
        self.0
    }
}

impl Deck for ClientDeck {
    fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

impl Debug for ClientDeck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}
