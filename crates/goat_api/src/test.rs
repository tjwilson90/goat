use core::mem;
use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;

use itertools::Itertools;

use crate::{
    Card, Cards, ClientGame, ClientPhase, ClientRummyHand, ClientWarHand, PlayerIdx, Rank,
    RummyPhase, RummyTrick, ServerGame, ServerPhase, ServerWarHand, Suit, WarPhase, WarPlayKind,
    WarTrick,
};

macro_rules! c {
    ($($cards:tt)*) => {
        stringify!($($cards)*).parse::<Cards>().unwrap()
    };
}

#[test]
fn card_display() {
    assert_eq!(Card::NineSpades.to_string(), "9S");
    assert_eq!(Card::ThreeDiamonds.to_string(), "3D");
    assert_eq!(Card::JackClubs.to_string(), "JC");
    assert_eq!(Card::AceHearts.to_string(), "AH");
}

#[test]
fn card_suit() {
    assert_eq!(Card::TwoClubs.suit(), Suit::Clubs);
    assert_eq!(Card::AceClubs.suit(), Suit::Clubs);
    assert_eq!(Card::TwoDiamonds.suit(), Suit::Diamonds);
    assert_eq!(Card::AceDiamonds.suit(), Suit::Diamonds);
    assert_eq!(Card::TwoHearts.suit(), Suit::Hearts);
    assert_eq!(Card::AceHearts.suit(), Suit::Hearts);
    assert_eq!(Card::TwoSpades.suit(), Suit::Spades);
    assert_eq!(Card::AceSpades.suit(), Suit::Spades);
}

#[test]
fn cards_range() {
    assert_eq!(
        Cards::range(Card::FiveDiamonds, Card::EightDiamonds),
        Card::FiveDiamonds + Card::SixDiamonds + Card::SevenDiamonds + Card::EightDiamonds
    );
}

#[test]
fn cards_display() {
    assert_eq!(
        format!(
            "{}",
            Card::NineSpades + Card::QueenSpades + Card::JackDiamonds
        ),
        "[Q9S JD]"
    );
}

#[test]
fn cards_max() {
    assert_eq!((Card::TwoClubs + Card::NineClubs).max(), Card::NineClubs);
    assert_eq!(
        (Card::FourHearts + Card::SevenDiamonds).max(),
        Card::FourHearts
    );
    assert_eq!((Card::AceSpades + Card::FiveSpades).max(), Card::AceSpades);
    assert_eq!(Cards::from(Card::FiveHearts).max(), Card::FiveHearts);
}

#[test]
fn cards_iter() {
    assert_eq!(
        (Card::QueenSpades + Card::AceHearts + Card::TenClubs + Card::JackDiamonds)
            .into_iter()
            .collect::<Vec<_>>(),
        vec![
            Card::QueenSpades,
            Card::AceHearts,
            Card::JackDiamonds,
            Card::TenClubs
        ]
    );
}

#[test]
fn cards_parse() {
    assert_eq!(Cards::from(Card::AceHearts), c!(AH))
}

#[test]
fn cards_contains() {
    let cards = Cards::CLUBS + Cards::CLUBS + Card::ThreeClubs + Card::FourClubs;
    for cards in cards.into_iter().combinations(5) {
        let compact: Cards = cards.iter().cloned().collect();
        for card in Cards::ONE_DECK.into_iter() {
            assert_eq!(cards.contains(&card), compact.contains(card));
        }
    }
}

#[test]
fn cards_contains_any() {
    let left = c!(88777S 42H AAD).into_iter();
    let right = c!(88777S 42H AAD).into_iter();
    for left in left.powerset() {
        let left = Cards::from_iter(left);
        let left_set: HashSet<_> = left.into_iter().collect();
        for right in right.clone().powerset() {
            let right = Cards::from_iter(right);
            assert_eq!(
                left.contains_any(right),
                right.into_iter().any(|c| left_set.contains(&c)),
                "left={}, right={}",
                left,
                right,
            );
        }
    }
}

#[test]
fn cards_contains_all() {
    let left = c!(88777S 42H AAD).into_iter();
    let right = c!(88777S 42H AAD).into_iter();
    for left in left.powerset() {
        let left = Cards::from_iter(left);
        let left_map: HashMap<Card, usize> = left.into_iter().fold(HashMap::new(), |mut m, c| {
            *m.entry(c).or_default() += 1;
            m
        });
        for right in right.clone().powerset() {
            let right = Cards::from_iter(right);
            let right_map: HashMap<Card, usize> =
                right.into_iter().fold(HashMap::new(), |mut m, c| {
                    *m.entry(c).or_default() += 1;
                    m
                });
            assert_eq!(
                left.contains_all(right),
                right
                    .into_iter()
                    .all(|c| left_map.get(&c).unwrap_or(&0) >= right_map.get(&c).unwrap_or(&0)),
                "left={}, right={}, left_map={:?}, right_map={:?}",
                left,
                right,
                left_map,
                right_map,
            );
        }
    }
}

#[test]
fn war_trick_rank_winner_next() {
    fn next(t: &mut WarTrick, c: Card) -> (Option<Rank>, Option<u8>, u8) {
        t.play(WarPlayKind::PlayHand, c);
        (t.rank(), t.winner().map(|i| i.0), t.next_player().0)
    }
    let mut t = WarTrick::new(PlayerIdx(1), 4);
    assert_eq!(
        (t.rank(), t.winner(), t.next_player()),
        (None, None, PlayerIdx(1))
    );
    assert_eq!(next(&mut t, Card::FiveSpades), (Some(Rank::Five), None, 2));
    assert_eq!(next(&mut t, Card::ThreeClubs), (Some(Rank::Five), None, 3));
    assert_eq!(next(&mut t, Card::EightClubs), (Some(Rank::Eight), None, 0));
    assert_eq!(next(&mut t, Card::EightClubs), (None, None, 3));
    assert_eq!(next(&mut t, Card::AceClubs), (Some(Rank::Ace), None, 0));
    assert_eq!(next(&mut t, Card::FourDiamonds), (None, Some(3), 3));
}

#[test]
fn war_trick_can_slough() {
    let num_players = 3;
    let mut t = WarTrick::new(PlayerIdx(2), num_players as usize);
    for card in Cards::ONE_DECK {
        for player in 0..num_players {
            assert!(!t.can_slough(PlayerIdx(player), card));
        }
    }
    t.play(WarPlayKind::PlayHand, Card::TwoClubs);
    for card in Cards::ONE_DECK {
        for player in 0..num_players {
            assert_eq!(
                t.can_slough(PlayerIdx(player), card),
                player == 2 && card.rank() == Rank::Two
            );
        }
    }
    t.play(WarPlayKind::PlayHand, Card::FiveClubs);
    for card in Cards::ONE_DECK {
        for player in 0..num_players {
            assert_eq!(
                t.can_slough(PlayerIdx(player), card),
                card.rank() == Rank::Two
                    || (card.rank() == Rank::Five && (player == 0 || player == 2))
            );
        }
    }
    t.play(WarPlayKind::PlayHand, Card::FiveDiamonds);
    for card in Cards::ONE_DECK {
        for player in 0..num_players {
            assert_eq!(
                t.can_slough(PlayerIdx(player), card),
                card.rank() == Rank::Two || card.rank() == Rank::Five
            );
        }
    }
    t.play(WarPlayKind::PlayHand, Card::ThreeClubs);
    for card in Cards::ONE_DECK {
        for player in 0..num_players {
            assert_eq!(
                t.can_slough(PlayerIdx(player), card),
                card.rank() == Rank::Two
                    || card.rank() == Rank::Five
                    || (card.rank() == Rank::Three && player != 1),
            );
        }
    }
}

#[test]
fn rummy_trick() {
    let mut t = RummyTrick::new(4);
    assert!(t.is_empty());
    assert!(t.can_play(Card::TwoClubs, Suit::Spades));
    assert!(!t.play(Card::ThreeClubs, Card::FourClubs));
    assert!(!t.is_empty());
    assert!(!t.can_play(Card::TwoClubs, Suit::Spades));
    assert!(!t.can_play(Card::FourClubs, Suit::Spades));
    assert!(t.can_play(Card::FiveClubs, Suit::Spades));
    assert!(!t.can_play(Card::FiveDiamonds, Suit::Spades));
    assert!(t.can_play(Card::TwoSpades, Suit::Spades));
    assert!(t.can_play(Card::FiveSpades, Suit::Spades));
    assert_eq!((Card::ThreeClubs, Card::FourClubs), t.pick_up());
    assert!(t.is_empty());
    assert!(!t.play(Card::ThreeClubs, Card::FourClubs));
    assert!(!t.play(Card::FiveClubs, Card::FiveClubs));
    assert!(!t.is_empty());
    assert_eq!((Card::ThreeClubs, Card::FiveClubs), t.pick_up());
    assert!(t.is_empty());
    assert!(!t.play(Card::ThreeClubs, Card::FourClubs));
    assert!(!t.play(Card::SixClubs, Card::SixClubs));
    assert!(!t.play(Card::FourSpades, Card::SixSpades));
    assert!(t.play(Card::EightSpades, Card::EightSpades));
}

#[test]
fn size_of() {
    assert_eq!(mem::size_of::<ClientGame<()>>(), 160);
    assert_eq!(mem::size_of::<ClientPhase<()>>(), 136);
    assert_eq!(mem::size_of::<WarPhase<u8, ClientWarHand, ()>>(), 128);
    assert_eq!(mem::size_of::<WarTrick>(), 88);
    assert_eq!(mem::size_of::<RummyPhase<ClientRummyHand>>(), 72);

    assert_eq!(mem::size_of::<ServerGame>(), 208);
    assert_eq!(mem::size_of::<ServerPhase>(), 152);
    assert_eq!(
        mem::size_of::<WarPhase<Vec<Card>, ServerWarHand, ()>>(),
        144
    );
    assert_eq!(mem::size_of::<RummyPhase<Cards>>(), 72);
}
