use std::convert::TryFrom;
use std::fmt::{Debug, Display, Write};
use std::{fmt, mem};

use crate::{Card, Rank};

const SUITS: [char; 4] = ['C', 'D', 'H', 'S'];

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub const VALUES: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];

    pub fn idx(self) -> usize {
        self as usize
    }

    pub fn char(self) -> char {
        SUITS[self.idx()]
    }

    pub fn with_rank(self, rank: Rank) -> Card {
        Card::new(rank, self)
    }
}

impl From<u8> for Suit {
    fn from(n: u8) -> Self {
        debug_assert!(n < 4, "n={}", n);
        unsafe { mem::transmute(n) }
    }
}

impl TryFrom<char> for Suit {
    type Error = char;

    fn try_from(c: char) -> Result<Self, Self::Error> {
        SUITS
            .iter()
            .position(|&s| s == c)
            .map(|n| Self::from(n as u8))
            .ok_or(c)
    }
}

impl Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char(self.char())
    }
}

impl Debug for Suit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}
