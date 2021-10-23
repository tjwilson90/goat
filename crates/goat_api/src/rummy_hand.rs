use std::fmt;
use std::fmt::Debug;
use std::ops::{AddAssign, SubAssign};

use crate::{Card, Cards, GoatError};

pub trait RummyHand: AddAssign<Card> + AddAssign<Cards> + SubAssign<Cards> {
    fn is_empty(&self) -> bool;

    fn check_can_play(&self, lo: Card, hi: Card) -> Result<(), GoatError>;
}

impl RummyHand for Cards {
    fn is_empty(&self) -> bool {
        Cards::is_empty(*self)
    }

    fn check_can_play(&self, lo: Card, hi: Card) -> Result<(), GoatError> {
        let cards = Cards::range(lo, hi);
        if !self.contains_all(cards) {
            for card in cards {
                if !self.contains(card) {
                    return Err(GoatError::NotYourCard { card });
                }
            }
        }
        Ok(())
    }
}

pub struct ClientRummyHand {
    pub known: Cards,
    pub unknown: u8,
}

impl RummyHand for ClientRummyHand {
    fn is_empty(&self) -> bool {
        self.known.is_empty() && self.unknown == 0
    }

    fn check_can_play(&self, _: Card, _: Card) -> Result<(), GoatError> {
        Ok(())
    }
}

impl AddAssign<Card> for ClientRummyHand {
    fn add_assign(&mut self, rhs: Card) {
        self.known += rhs;
    }
}

impl AddAssign<Cards> for ClientRummyHand {
    fn add_assign(&mut self, rhs: Cards) {
        self.known += rhs;
    }
}

impl SubAssign<Cards> for ClientRummyHand {
    fn sub_assign(&mut self, rhs: Cards) {
        if self.known.contains_all(rhs) {
            self.known -= rhs;
        } else {
            for card in rhs {
                if self.known.contains(card) {
                    self.known -= card;
                } else {
                    self.unknown -= 1;
                }
            }
        }
    }
}

impl Debug for ClientRummyHand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.known, f)?;
        if self.unknown != 0 {
            f.write_str(" + ")?;
            Debug::fmt(&self.unknown, f)?;
        }
        Ok(())
    }
}
