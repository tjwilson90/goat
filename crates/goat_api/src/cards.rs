use std::convert::{Infallible, TryFrom};
use std::fmt;
use std::fmt::{Debug, Display, Write};
use std::iter::FromIterator;
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};
use std::str::FromStr;

use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Card, Rank, Suit};

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Cards {
    pub bits: u128,
}

impl Serialize for Cards {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for card in *self {
            seq.serialize_element(&card)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Cards {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(CardsVisitor(Cards::NONE))
    }
}

struct CardsVisitor(Cards);

impl<'de> Visitor<'de> for CardsVisitor {
    type Value = Cards;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(formatter, "a sequence of cards")
    }

    fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while let Some(card) = seq.next_element::<Card>()? {
            self.0 += card;
        }
        Ok(self.0)
    }
}

impl Cards {
    pub const NONE: Cards = Cards {
        bits: 0x0000_0000_0000_0000_0000_0000_0000_0000,
    };
    pub const CLUBS: Cards = Cards {
        bits: 0x0000_0000_0000_0000_0000_0000_0155_5555,
    };
    pub const ONE_DECK: Cards = Cards {
        bits: 0x0155_5555_0155_5555_0155_5555_0155_5555,
    };
    pub const COMMON_DRECK: Cards = Cards {
        bits: 0x0000_0155_0000_0155_0000_0155_0000_0155,
    };

    pub fn range(lo: Card, hi: Card) -> Cards {
        let lo_bits = Cards::from(lo).bits;
        let hi_bits = Cards::from(hi).bits;
        let range_bits = 2 * hi_bits - lo_bits;
        Cards {
            bits: range_bits & Self::ONE_DECK.bits,
        }
    }

    pub fn is_empty(self) -> bool {
        self == Self::NONE
    }

    pub fn len(self) -> usize {
        let hi = (self.bits & (Self::ONE_DECK.bits << 1)).count_ones();
        let lo = (self.bits & Self::ONE_DECK.bits).count_ones();
        (2 * hi + lo) as usize
    }

    pub fn max(self) -> Card {
        Card::from((63 - self.bits.leading_zeros() / 2) as u8)
    }

    pub fn min(self) -> Card {
        Card::from((self.bits.trailing_zeros() / 2) as u8)
    }

    pub fn in_suit(self, suit: Suit) -> Cards {
        let mask = 0x0155_5555 << (32 * suit.idx());
        Cards {
            bits: self.bits & mask,
        }
    }

    pub fn above(self, card: Card) -> Cards {
        let suit_mask = 0x0155_5555 << (32 * card.suit().idx());
        let rank_mask = !(2 * Cards::from(card).bits - 1);
        Cards {
            bits: self.bits & suit_mask & rank_mask,
        }
    }

    pub fn below(self, card: Card) -> Cards {
        let suit_mask = 0x0155_5555 << (32 * card.suit().idx());
        let rank_mask = Cards::from(card).bits - 1;
        Cards {
            bits: self.bits & suit_mask & rank_mask,
        }
    }

    pub fn contains(self, other: Card) -> bool {
        let mask = 0b11_u128 << (other as u8 * 2);
        (self.bits & mask) != 0
    }

    pub fn contains_any(self, other: Cards) -> bool {
        let mut mask = (other.bits | (other.bits >> 1)) & Self::ONE_DECK.bits;
        mask |= mask << 1;
        (self.bits & mask) != 0
    }

    pub fn contains_all(self, other: Cards) -> bool {
        let diff = Cards {
            bits: self.bits.wrapping_sub(other.bits),
        };
        self.len().wrapping_sub(other.len()) == diff.len()
    }

    pub fn remove_all(&mut self, cards: Cards) -> Cards {
        let mut mask = (cards.bits | (cards.bits >> 1)) & Self::ONE_DECK.bits;
        mask |= mask << 1;
        let removed = self.bits & mask;
        self.bits -= removed;
        Cards { bits: removed }
    }
}

impl Display for Cards {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("[")?;
        let mut iter = self.into_iter();
        let card = match iter.next() {
            Some(card) => card,
            None => return f.write_str("]"),
        };
        Display::fmt(&card.rank(), f)?;
        let mut prev_suit = card.suit();
        for card in iter {
            if card.suit() != prev_suit {
                Display::fmt(&prev_suit, f)?;
                f.write_char(' ')?;
            }
            Display::fmt(&card.rank(), f)?;
            prev_suit = card.suit();
        }
        Display::fmt(&prev_suit, f)?;
        f.write_str("]")?;
        Ok(())
    }
}

impl Debug for Cards {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl FromStr for Cards {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut cards = Cards::NONE;
        let mut chars = s.chars();
        let mut curr_suit = Suit::Clubs;
        while let Some(c) = chars.next_back() {
            if let Ok(rank) = Rank::try_from(c) {
                cards += Card::new(rank, curr_suit);
            } else if let Ok(suit) = Suit::try_from(c) {
                curr_suit = suit;
            }
        }
        Ok(cards)
    }
}

impl From<Card> for Cards {
    fn from(card: Card) -> Self {
        Cards {
            bits: 1 << (2 * (card as u8)),
        }
    }
}

impl Add<Cards> for Cards {
    type Output = Self;

    fn add(self, rhs: Cards) -> Self::Output {
        let sum = Cards {
            bits: self.bits + rhs.bits,
        };
        debug_assert!(sum.len() == self.len() + rhs.len());
        sum
    }
}

impl Add<Card> for Cards {
    type Output = Self;

    fn add(self, rhs: Card) -> Self::Output {
        self + Self::from(rhs)
    }
}

impl AddAssign<Cards> for Cards {
    fn add_assign(&mut self, rhs: Cards) {
        *self = *self + rhs
    }
}

impl AddAssign<Card> for Cards {
    fn add_assign(&mut self, rhs: Card) {
        *self = *self + rhs
    }
}

impl Sub<Cards> for Cards {
    type Output = Self;

    fn sub(self, rhs: Cards) -> Self::Output {
        debug_assert!(self.contains_all(rhs));
        Cards {
            bits: self.bits - rhs.bits,
        }
    }
}

impl Sub<Card> for Cards {
    type Output = Self;

    fn sub(self, rhs: Card) -> Self::Output {
        self - Self::from(rhs)
    }
}

impl SubAssign<Cards> for Cards {
    fn sub_assign(&mut self, rhs: Cards) {
        *self = *self - rhs
    }
}

impl SubAssign<Card> for Cards {
    fn sub_assign(&mut self, rhs: Card) {
        *self = *self - rhs;
    }
}

impl Mul<usize> for Cards {
    type Output = Cards;

    fn mul(self, rhs: usize) -> Self::Output {
        debug_assert!(rhs <= 3);
        (0..rhs).fold(Cards::NONE, |c, _| c + self)
    }
}

impl IntoIterator for Cards {
    type Item = Card;
    type IntoIter = CardsIter;

    fn into_iter(self) -> Self::IntoIter {
        CardsIter(self)
    }
}

#[derive(Clone)]
pub struct CardsIter(Cards);

impl Iterator for CardsIter {
    type Item = Card;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == Cards::NONE {
            None
        } else {
            let card = self.0.max();
            self.0 -= card;
            Some(card)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.0.len() as usize;
        (size, Some(size))
    }
}

impl ExactSizeIterator for CardsIter {}

impl FromIterator<Card> for Cards {
    fn from_iter<T: IntoIterator<Item = Card>>(iter: T) -> Self {
        iter.into_iter().fold(Cards::NONE, Cards::add)
    }
}

impl FromIterator<Cards> for Cards {
    fn from_iter<T: IntoIterator<Item = Cards>>(iter: T) -> Self {
        iter.into_iter().fold(Cards::NONE, Cards::add)
    }
}

impl Extend<Card> for Cards {
    fn extend<T: IntoIterator<Item = Card>>(&mut self, iter: T) {
        iter.into_iter().fold(self, |cards, card| {
            *cards += card;
            cards
        });
    }
}
