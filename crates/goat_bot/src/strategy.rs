use goat_api::{
    Action, Card, Cards, ClientDeck, ClientRummyHand, ClientWarHand, Deck, PlayerIdx, Rank,
    RummyHand, RummyPhase, Suit, WarHand, WarPhase,
};

pub trait Strategy {
    fn war(&self, idx: PlayerIdx, war: &WarPhase<ClientDeck, ClientWarHand>) -> Option<Action>;
    fn rummy(&self, idx: PlayerIdx, rummy: &RummyPhase<ClientRummyHand>) -> Action;
}

/// The simplest possible war strategy, never hold any cards in hand and always play from the top
/// of the deck.
pub fn war_play_top(idx: PlayerIdx, war: &WarPhase<ClientDeck, ClientWarHand>) -> Option<Action> {
    if war.trick.next_player() == idx {
        Some(Action::PlayTop)
    } else {
        None
    }
}

/// A war strategy that tries to lose every trick.
pub fn war_duck(idx: PlayerIdx, war: &WarPhase<ClientDeck, ClientWarHand>) -> Option<Action> {
    let hand = match &war.hands[idx.idx()] {
        ClientWarHand::Visible(hand) => hand,
        _ => panic!("bot hand is hidden"),
    };
    if hand.len() < 3 && !war.deck.is_empty() {
        return Some(Action::Draw);
    }
    for card in hand.cards() {
        if card.rank() > Rank::Eight && war.trick.can_slough(idx, card) {
            return Some(Action::Slough { card });
        }
    }
    if war.trick.next_player() != idx {
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
        } else if !war.deck.is_empty() {
            Some(Action::PlayTop)
        } else {
            let card = hand.cards().min_by_key(|c| c.rank()).unwrap();
            Some(Action::PlayCard { card })
        }
    } else {
        let card = hand.cards().min_by_key(|c| c.rank()).unwrap();
        if war.deck.is_empty() || card.rank() < Rank::Six {
            Some(Action::PlayCard { card })
        } else {
            Some(Action::PlayTop)
        }
    }
}

/// A war strategy that tries to win every trick.
pub fn war_cover(idx: PlayerIdx, war: &WarPhase<ClientDeck, ClientWarHand>) -> Option<Action> {
    let hand = match &war.hands[idx.idx()] {
        ClientWarHand::Visible(hand) => hand,
        _ => panic!("bot hand is hidden"),
    };
    if hand.len() < 3 && !war.deck.is_empty() {
        return Some(Action::Draw);
    }
    for card in hand.cards() {
        if card.rank() < Rank::Eight && war.trick.can_slough(idx, card) {
            return Some(Action::Slough { card });
        }
    }
    if war.trick.next_player() != idx {
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
        } else if !war.deck.is_empty() {
            Some(Action::PlayTop)
        } else {
            let card = hand.cards().min_by_key(|c| c.rank()).unwrap();
            Some(Action::PlayCard { card })
        }
    } else {
        let card = hand.cards().max_by_key(|c| c.rank()).unwrap();
        if war.deck.is_empty() || card.rank() > Rank::Ten {
            Some(Action::PlayCard { card })
        } else {
            Some(Action::PlayTop)
        }
    }
}

/// A rummy strategy that preferentially plays long, low runs from suits with many runs.
pub fn rummy_simple(idx: PlayerIdx, rummy: &RummyPhase<ClientRummyHand>) -> Action {
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
                let (lo, hi) = lowest_run(above);
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
                        return (0, 13);
                    }
                    let in_suit = hand.in_suit(*s);
                    if in_suit.is_empty() {
                        (0, 0)
                    } else {
                        (
                            distinct_runs(in_suit),
                            13 - distinct_runs(prev.known.below(in_suit.min())),
                        )
                    }
                })
                .unwrap();
            let (lo, hi) = lowest_run(hand.in_suit(suit));
            Action::PlayRun { lo, hi }
        }
    }
}

fn distinct_runs(mut hand: Cards) -> usize {
    let mut count = 0;
    while !hand.is_empty() {
        count += 1;
        let (lo, hi) = lowest_run(hand);
        hand -= Cards::range(lo, hi);
    }
    count
}

fn lowest_run(hand: Cards) -> (Card, Card) {
    let lo = hand.min();
    (lo, top_of_run(hand, lo))
}

fn top_of_run(hand: Cards, lo: Card) -> Card {
    let mut hi = lo;
    while hi.rank() < Rank::Ace {
        let next = hi.with_rank(hi.rank().next_up());
        if hand.contains(next) {
            hi = next;
        } else {
            break;
        }
    }
    hi
}
