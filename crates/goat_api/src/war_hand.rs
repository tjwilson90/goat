use std::fmt;
use std::fmt::Debug;
use std::ops::{AddAssign, SubAssign};

use crate::{Card, Cards, ClientRummyHand, GoatError, RummyHand};

pub trait WarHand: AddAssign<Card> + SubAssign<Card> + Debug {
    type RummyHand: RummyHand;

    fn new() -> Self;

    fn is_empty(&self) -> bool;

    fn len(&self) -> usize;

    fn check_has_card(&self, card: Card) -> Result<(), GoatError>;

    fn merge_into_rummy_hand(&self, won: Cards) -> Self::RummyHand;
}

#[derive(Clone)]
pub struct ServerWarHand {
    cards: [Option<Card>; 3],
}

impl ServerWarHand {
    pub fn cards(&self) -> impl Iterator<Item = Card> + '_ {
        self.cards.iter().filter_map(|c| *c)
    }
}

impl WarHand for ServerWarHand {
    type RummyHand = Cards;

    fn new() -> Self {
        Self { cards: [None; 3] }
    }

    fn is_empty(&self) -> bool {
        self.cards[0].is_none()
    }

    fn len(&self) -> usize {
        for i in 0..3 {
            if self.cards[i].is_none() {
                return i;
            }
        }
        3
    }

    fn check_has_card(&self, card: Card) -> Result<(), GoatError> {
        if self.cards.contains(&Some(card)) {
            Ok(())
        } else {
            Err(GoatError::NotYourCard { card })
        }
    }

    fn merge_into_rummy_hand(&self, mut won: Cards) -> Self::RummyHand {
        won.extend(self.cards());
        won
    }
}

impl AddAssign<Card> for ServerWarHand {
    fn add_assign(&mut self, rhs: Card) {
        for i in 0..3 {
            if self.cards[i].is_none() {
                self.cards[i] = Some(rhs);
                return;
            }
        }
    }
}

impl SubAssign<Card> for ServerWarHand {
    fn sub_assign(&mut self, rhs: Card) {
        let idx = self.cards.iter().position(|c| *c == Some(rhs)).unwrap();
        #[allow(clippy::suspicious_op_assign_impl)]
        self.cards.copy_within(idx + 1.., idx);
        self.cards[2] = None;
    }
}

impl Debug for ServerWarHand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        let mut cards = self.cards();
        if let Some(card) = cards.next() {
            Debug::fmt(&card, f)?;
        }
        for card in cards {
            f.write_str(",")?;
            Debug::fmt(&card, f)?;
        }
        f.write_str("]")
    }
}

#[derive(Clone)]
pub enum ClientWarHand {
    Visible(ServerWarHand),
    Hidden(u8),
}

impl WarHand for ClientWarHand {
    type RummyHand = ClientRummyHand;

    fn new() -> Self {
        Self::Visible(ServerWarHand::new())
    }

    fn is_empty(&self) -> bool {
        match self {
            ClientWarHand::Visible(hand) => hand.is_empty(),
            ClientWarHand::Hidden(len) => *len == 0,
        }
    }

    fn len(&self) -> usize {
        match self {
            ClientWarHand::Visible(hand) => hand.len(),
            ClientWarHand::Hidden(len) => *len as usize,
        }
    }

    fn check_has_card(&self, _: Card) -> Result<(), GoatError> {
        Ok(())
    }

    fn merge_into_rummy_hand(&self, won: Cards) -> Self::RummyHand {
        match self {
            ClientWarHand::Visible(hand) => ClientRummyHand {
                known: hand.merge_into_rummy_hand(won),
                unknown: 0,
            },
            ClientWarHand::Hidden(len) => ClientRummyHand {
                known: won,
                unknown: *len,
            },
        }
    }
}

impl AddAssign<Card> for ClientWarHand {
    fn add_assign(&mut self, rhs: Card) {
        match self {
            ClientWarHand::Visible(hand) => {
                *hand += rhs;
            }
            ClientWarHand::Hidden(0) => {
                let mut hand = ServerWarHand::new();
                hand += rhs;
                *self = ClientWarHand::Visible(hand);
            }
            _ => panic!("cannot add {} to hidden hand", rhs),
        }
    }
}

impl SubAssign<Card> for ClientWarHand {
    fn sub_assign(&mut self, rhs: Card) {
        match self {
            ClientWarHand::Visible(hand) => {
                *hand -= rhs;
            }
            ClientWarHand::Hidden(count) => {
                *count -= 1;
            }
        }
    }
}

impl AddAssign<u8> for ClientWarHand {
    fn add_assign(&mut self, rhs: u8) {
        match self {
            ClientWarHand::Visible(hand) if hand.is_empty() => {
                *self = ClientWarHand::Hidden(rhs);
            }
            ClientWarHand::Hidden(count) => *count += rhs,
            _ => panic!("cannot add {} to visible hand", rhs),
        }
    }
}

impl Debug for ClientWarHand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientWarHand::Visible(hand) => Debug::fmt(hand, f),
            ClientWarHand::Hidden(len) => Debug::fmt(len, f),
        }
    }
}
