use std::convert::TryFrom;
use std::fmt::{Debug, Display, Write};
use std::{fmt, mem};

use serde::{Serialize, Serializer};

use crate::{Card, Suit};

const RANKS: [char; 13] = [
    '2', '3', '4', '5', '6', '7', '8', '9', 'T', 'J', 'Q', 'K', 'A',
];

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Rank {
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

impl Rank {
    pub fn char(self) -> char {
        RANKS[self as usize]
    }

    pub fn idx(self) -> usize {
        self as usize
    }

    pub fn with_suit(self, suit: Suit) -> Card {
        Card::new(self, suit)
    }

    pub fn next_up(self) -> Rank {
        Rank::from(self as u8 + 1)
    }
}

impl From<u8> for Rank {
    fn from(n: u8) -> Self {
        assert!(n < 13, "n={}", n);
        unsafe { mem::transmute(n) }
    }
}

impl TryFrom<char> for Rank {
    type Error = char;

    fn try_from(c: char) -> Result<Self, Self::Error> {
        RANKS
            .iter()
            .position(|&r| r == c)
            .map(|n| Self::from(n as u8))
            .ok_or(c)
    }
}

impl Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char(self.char())
    }
}

impl Debug for Rank {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Serialize for Rank {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_char(self.char())
    }
}
