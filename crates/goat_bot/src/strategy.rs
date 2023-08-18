use goat_api::{
    Action, Cards, ClientDeck, ClientGame, ClientPhase, ClientRummyHand, ClientWarHand, Deck,
    Event, PlayerIdx, Rank, RummyHand, Suit, WarHand,
};
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::HashMap;
use std::time::{Duration, Instant};

type WarPhase = goat_api::WarPhase<ClientDeck, ClientWarHand, ()>;
type RummyPhase = goat_api::RummyPhase<ClientRummyHand, Cards>;

pub trait Strategy: Send + Sync + 'static {
    fn war(&self, idx: PlayerIdx, war: &WarPhase) -> Option<Action>;
    fn rummy(&self, rummy: &RummyPhase) -> Action;
}

/// The simplest possible war strategy, never hold any cards in hand and always play from the top
/// of the deck.
pub fn war_play_top(idx: PlayerIdx, war: &WarPhase) -> Option<Action> {
    if war.is_finished() || war.trick.winner().is_some() {
        return if war.trick.ended(idx) {
            None
        } else {
            Some(Action::FinishTrick)
        };
    }
    if war.trick.next_player() == Some(idx) {
        Some(Action::PlayTop)
    } else {
        None
    }
}

/// A war strategy that tries to lose every trick.
pub fn war_duck(idx: PlayerIdx, war: &WarPhase) -> Option<Action> {
    let hand = match &war.hands[idx.idx()] {
        ClientWarHand::Visible(hand) => hand,
        _ => panic!("bot hand is hidden"),
    };
    if hand.len() < 3 && war.deck.cards_remaining() > 0 {
        return Some(Action::Draw);
    }
    for card in hand.cards() {
        if card.rank() > Rank::Eight && war.trick.check_can_slough(idx, hand, card).is_ok() {
            return Some(Action::Slough { card });
        }
    }
    if war.is_finished() || war.trick.winner().is_some() {
        return if war.trick.ended(idx) {
            None
        } else {
            Some(Action::FinishTrick)
        };
    }
    if war.trick.next_player() != Some(idx) {
        None
    } else if let Some(rank) = war.trick.rank() {
        if let Some(card) = hand.cards().find(|c| c.rank() == rank) {
            Some(Action::PlayCard { card })
        } else if let Some(card) = hand
            .cards()
            .filter(|c| c.rank() < rank)
            .max_by_key(|c| c.rank())
        {
            Some(Action::PlayCard { card })
        } else if war.deck.cards_remaining() > 0 {
            Some(Action::PlayTop)
        } else {
            let card = hand.cards().min_by_key(|c| c.rank()).unwrap();
            Some(Action::PlayCard { card })
        }
    } else {
        let card = hand.cards().min_by_key(|c| c.rank()).unwrap();
        if war.deck.cards_remaining() == 0 || card.rank() < Rank::Six {
            Some(Action::PlayCard { card })
        } else {
            Some(Action::PlayTop)
        }
    }
}

/// A war strategy that tries to win every trick.
pub fn war_cover(idx: PlayerIdx, war: &WarPhase) -> Option<Action> {
    let hand = match &war.hands[idx.idx()] {
        ClientWarHand::Visible(hand) => hand,
        _ => panic!("bot hand is hidden"),
    };
    if hand.len() < 3 && war.deck.cards_remaining() > 0 {
        return Some(Action::Draw);
    }
    for card in hand.cards() {
        if card.rank() < Rank::Eight && war.trick.check_can_slough(idx, hand, card).is_ok() {
            return Some(Action::Slough { card });
        }
    }
    if war.is_finished() || war.trick.winner().is_some() {
        return if war.trick.ended(idx) {
            None
        } else {
            Some(Action::FinishTrick)
        };
    }
    if war.trick.next_player() != Some(idx) {
        None
    } else if let Some(rank) = war.trick.rank() {
        if let Some(card) = hand.cards().find(|c| c.rank() == rank) {
            Some(Action::PlayCard { card })
        } else if let Some(card) = hand
            .cards()
            .filter(|c| c.rank() > rank)
            .min_by_key(|c| c.rank())
        {
            Some(Action::PlayCard { card })
        } else if war.deck.cards_remaining() > 0 {
            Some(Action::PlayTop)
        } else {
            let card = hand.cards().min_by_key(|c| c.rank()).unwrap();
            Some(Action::PlayCard { card })
        }
    } else {
        let card = hand.cards().max_by_key(|c| c.rank()).unwrap();
        if war.deck.cards_remaining() == 0 || card.rank() > Rank::Ten {
            Some(Action::PlayCard { card })
        } else {
            Some(Action::PlayTop)
        }
    }
}

/// A rummy strategy that preferentially plays long, low runs from suits with many runs.
pub fn rummy_simple(rummy: &RummyPhase) -> Action {
    let idx = rummy.next;
    let hand = rummy.hands[idx.idx()].known;
    match rummy.trick.top_card() {
        Some(card) => {
            let trump_suit = rummy.trump.suit();
            let above = hand.above(card);
            let trump = hand.in_suit(trump_suit);
            if above.is_empty() && (trump.is_empty() || card.suit() == trump_suit) {
                Action::PickUp
            } else if card.suit() == trump_suit {
                let low_trump = above.min();
                Action::PlayRun {
                    lo: low_trump,
                    hi: low_trump,
                }
            } else if above.is_empty() {
                let low_trump = trump.min();
                Action::PlayRun {
                    lo: low_trump,
                    hi: low_trump,
                }
            } else {
                let (lo, hi) = above.min_run();
                Action::PlayRun { lo, hi }
            }
        }
        None => {
            let mut prev = if idx.idx() == 0 {
                rummy.hands.len() - 1
            } else {
                idx.idx() - 1
            };
            while rummy.hands[prev].is_empty() {
                prev = if prev == 0 {
                    rummy.hands.len() - 1
                } else {
                    prev - 1
                };
            }
            let prev = &rummy.hands[prev];
            let suit = Suit::VALUES
                .iter()
                .cloned()
                .max_by_key(|s| {
                    if *s == rummy.trump.suit() {
                        return (0, 100);
                    }
                    let in_suit = hand.in_suit(*s);
                    if in_suit.is_empty() {
                        (0, 0)
                    } else {
                        (
                            in_suit.runs().count(),
                            100 - prev.known.below(in_suit.min()).runs().count(),
                        )
                    }
                })
                .unwrap();
            let (lo, hi) = hand.in_suit(suit).min_run();
            Action::PlayRun { lo, hi }
        }
    }
}

pub fn rummy_random(rummy: &RummyPhase) -> Action {
    let hand = rummy.hands[rummy.next.idx()].known;
    let trump = rummy.trump.suit();
    let trumps = hand.in_suit(trump);
    let plays = match rummy.trick.top_card() {
        Some(card) => {
            if rummy.trick.num_players() >= 4 && rummy.trick.plays()[0].0.suit() == trump {
                return Action::PickUp;
            }
            let above = hand.above(card);
            if card.suit() == trump {
                above
            } else {
                above + trumps
            }
        }
        None => {
            if hand == trumps || rummy.trick.num_players() == 2 {
                hand
            } else {
                hand - trumps
            }
        }
    };
    if plays.is_empty() {
        return Action::PickUp;
    }
    let has_excessive_trump = plays.in_suit(trump).len() > 1 && {
        let mut non_trump_runs = 0;
        for (lo, _) in hand.runs() {
            if lo.suit() != trump {
                non_trump_runs += 1;
            }
        }
        let remaining_trumps = trumps - Cards::from(trumps.min_run());
        non_trump_runs <= remaining_trumps.len()
    };
    let mut num_choices = rummy.trick.top_card().is_some() as usize;
    for (lo, hi) in plays.runs() {
        let could_play_whole_run = lo != hi && (has_excessive_trump || lo.suit() != trump);
        num_choices += 1 + could_play_whole_run as usize;
    }
    let mut choice = rand::thread_rng().gen_range(0..num_choices);
    for (lo, hi) in plays.runs() {
        let could_play_whole_run = lo != hi && (has_excessive_trump || lo.suit() != trump);
        if choice == 0 {
            return Action::PlayRun { lo, hi: lo };
        } else if choice == 1 && could_play_whole_run {
            return Action::PlayRun { lo, hi };
        }
        choice -= 1 + could_play_whole_run as usize;
    }
    debug_assert_eq!(choice, 0);
    Action::PickUp
}

pub fn rummy_simulate(rummy: &RummyPhase) -> Action {
    let mut unknown = Cards::ONE_DECK * 3;
    let mut count = 1 + rummy.history.len();
    unknown -= rummy.trump;
    unknown -= rummy.history;
    for hand in rummy.hands.iter() {
        unknown -= hand.known;
        count += hand.len();
    }
    if count % 52 != 0 {
        panic!("unexpected state: {:?}", rummy);
    }
    if count == 52 {
        unknown -= Cards::ONE_DECK * 2;
    } else if count == 104 {
        unknown -= Cards::ONE_DECK;
    }
    let start = Instant::now();
    let mut simulations = HashMap::new();
    while start.elapsed() < Duration::from_secs(3) {
        let (action, goat) = simulate_once(rummy.clone(), unknown);
        let (losses, games) = simulations.entry(action).or_insert((0, 0));
        *losses += (goat == rummy.next) as u64;
        *games += 1;
    }
    log::debug!("Simulations on {:?} produced {:?}", rummy, simulations);
    simulations
        .into_iter()
        .min_by_key(|(_, (losses, games))| {
            if *games == 0 {
                u32::MAX
            } else {
                ((*losses * (u32::MAX as u64)) / *games) as u32
            }
        })
        .map(|(action, _)| action)
        .unwrap()
}

fn simulate_once(mut rummy: RummyPhase, unknown: Cards) -> (Action, PlayerIdx) {
    if !unknown.is_empty() {
        let mut unknown: Vec<_> = unknown.cards().collect();
        unknown.shuffle(&mut rand::thread_rng());
        let mut unknown = unknown.into_iter();
        for hand in rummy.hands.iter_mut() {
            for _ in 0..hand.unknown {
                hand.known += unknown.next().unwrap();
            }
            hand.unknown = 0;
        }
    }
    let mut game = ClientGame {
        phase: ClientPhase::<(), Cards>::Rummy(rummy),
        players: vec![],
    };
    let mut first_action = None;
    loop {
        let action = match &game.phase {
            ClientPhase::Rummy(rummy) => rummy_random(rummy),
            ClientPhase::Goat(goat) => return (first_action.unwrap(), goat.goat),
            _ => panic!("unexpected phase"),
        };
        if first_action.is_none() {
            first_action = Some(action);
        }
        let event = match action {
            Action::PickUp => Event::PickUp,
            Action::PlayRun { lo, hi } => Event::PlayRun { lo, hi },
            _ => panic!("unexpected action"),
        };
        game.apply(event).unwrap();
    }
}
