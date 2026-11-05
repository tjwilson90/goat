//! Shared, suit-relative state/action features used by Python and Rust inference.
use crate::{learned::*, rummy_simple};
use goat_api::*;

pub const FEATURES: usize = 80;
pub const FEATURE_SCHEMA: u32 = 1;

fn cards(row: &[f32]) -> Cards {
    let mut result = Cards::NONE;
    for (i, &count) in row[..52].iter().enumerate() {
        for _ in 0..(count * 3.).round() as usize {
            result += Card::new(Rank::from((i % 13) as u8), Suit::from((i / 13) as u8));
        }
    }
    result
}

/// Lossless for valid rummy observations; seats are already relative to actor.
pub fn from_observation(obs: &[f32]) -> Option<Rummy> {
    let global = &obs[MAX_PLAYERS * SEAT_FEATURES..];
    if global[1] < 0.5 {
        return None;
    }
    let n = (global[2] * 15.).round() as usize;
    let mut pick_ups = 0u128;
    let hands = (0..n)
        .map(|i| {
            let row = &obs[i * SEAT_FEATURES..];
            pick_ups |= ((row[109] * 64.).round() as u128) << (8 * i);
            ClientRummyHand {
                public: Cards::NONE,
                known: cards(row),
                unknown: (row[105] * 156.).round() as u8,
            }
        })
        .collect();
    let mut trick = RummyTrick::new((global[8] * 15.).round() as usize);
    for i in 0..(global[9] * 15.).round() as usize {
        let row = &global[276 + i * 8..];
        let suit = Suit::from(row[2..6].iter().position(|&v| v > 0.5).unwrap() as u8);
        let lo = Card::new(Rank::from((row[0] * 13.).round() as u8 - 1), suit);
        let hi = Card::new(Rank::from((row[1] * 13.).round() as u8 - 1), suit);
        trick.play(PlayerIdx((row[6] * 15.).round() as u8), lo, hi);
    }
    Some(Rummy {
        hands,
        trick,
        next: PlayerIdx(0),
        trump: cards(&global[68..]).min(),
        pick_ups,
        history: cards(&global[16..]),
    })
}

pub fn features(view: &Rummy, action: usize, teacher: usize) -> [f32; FEATURES] {
    let mut f = [0.; FEATURES];
    let root = view.next.idx();
    let own = view.hands[root].known;
    let trump = view.trump.suit();
    let active: Vec<_> = (1..view.hands.len())
        .map(|i| (root + i) % view.hands.len())
        .filter(|&i| !view.hands[i].is_empty())
        .collect();
    let pickup = action == 0;
    let played = match decode_action(action).unwrap() {
        Action::PlayRun { lo, hi } => Cards::range(lo, hi),
        Action::PickUp => {
            let mut trick = view.trick.clone();
            let (lo, hi) = trick.pick_up();
            Cards::range(lo, hi)
        }
        _ => unreachable!(),
    };
    let lo = played.min();
    let hi = played.max();
    let suit = lo.suit();
    let remaining = if pickup { own + played } else { own - played };
    let is_trump = suit == trump;
    let leading = view.trick.is_empty();
    f[0] = (action == teacher) as u8 as f32;
    f[1] = pickup as u8 as f32;
    f[2] = is_trump as u8 as f32;
    f[3] = lo.rank().idx() as f32 / 12.;
    f[4] = hi.rank().idx() as f32 / 12.;
    f[5] = played.len() as f32 / 13.;
    f[6] = own.len() as f32 / 26.;
    f[7] = remaining.len() as f32 / 26.;
    f[8] = remaining.is_empty() as u8 as f32;
    f[9] = leading as u8 as f32;
    f[10] = (!pickup && view.trick.len() + 1 == view.trick.num_players()) as u8 as f32;
    f[11] = (active.len() + 1) as f32 / 6.;
    f[12] = view.trick.len() as f32 / 6.;
    f[13] = own.runs().count() as f32 / 13.;
    f[14] = remaining.runs().count() as f32 / 13.;
    f[15] = own.in_suit(suit).runs().count() as f32 / 13.;
    f[16] = remaining.in_suit(suit).runs().count() as f32 / 13.;
    f[17] = remaining.in_suit(trump).len() as f32 / 13.;
    f[18] = own.in_suit(suit).len() as f32 / 13.;
    f[19] = remaining.in_suit(suit).len() as f32 / 13.;
    f[20] = ((view.pick_ups >> (8 * root)) & 255) as f32 / 64.;
    f[21] = view.hands.iter().map(|h| h.len()).sum::<usize>() as f32 / 52.;
    f[22] = view.hands.iter().map(|h| h.unknown as usize).sum::<usize>() as f32 / 52.;
    if let Some(top) = view.trick.top_card() {
        f[23] = (top.suit() == trump) as u8 as f32;
        f[24] = top.rank().idx() as f32 / 12.;
    }
    f[25] = (remaining.runs().count() == 1) as u8 as f32;
    f[26] = (!own.in_suit(suit).is_empty() && own.in_suit(suit).min() == lo) as u8 as f32;
    // Card texture is relative to the proposed suit and the trump suit.
    for rank in 0..13 {
        f[28 + rank] = remaining.contains(Card::new(Rank::from(rank as u8), suit)) as u8 as f32;
        f[41 + rank] = remaining.contains(Card::new(Rank::from(rank as u8), trump)) as u8 as f32;
    }
    // Next and previous active opponents retain their distinct tactical roles.
    for (slot, index) in [active.first(), active.last()].into_iter().enumerate() {
        if let Some(&i) = index {
            let hand = &view.hands[i];
            let offset = 54 + 8 * slot;
            f[offset] = hand.len() as f32 / 26.;
            f[offset + 1] = hand.unknown as f32 / 26.;
            f[offset + 2] = hand.known.above(hi).len() as f32 / 13.;
            f[offset + 3] = hand.known.in_suit(trump).len() as f32 / 13.;
            f[offset + 4] = hand.known.below(lo).runs().count() as f32 / 13.;
            f[offset + 5] = hand.known.in_suit(suit).len() as f32 / 13.;
            f[offset + 6] = ((view.pick_ups >> (8 * i)) & 255) as f32 / 64.;
            f[offset + 7] = (hand.len() == 1) as u8 as f32;
        }
    }
    if !active.is_empty() {
        f[70] = active.iter().map(|&i| view.hands[i].len()).min().unwrap() as f32 / 26.;
        f[71] = active.iter().map(|&i| view.hands[i].len()).max().unwrap() as f32 / 26.;
        f[72] = active
            .iter()
            .filter(|&&i| {
                !view.hands[i].known.above(hi).is_empty()
                    || (!is_trump && !view.hands[i].known.in_suit(trump).is_empty())
            })
            .count() as f32
            / active.len() as f32;
    }
    // Connected pickup size, rather than assuming the whole trick is taken.
    if !leading {
        let mut trick = view.trick.clone();
        let (lo, hi) = trick.pick_up();
        f[73] = Cards::range(lo, hi).len() as f32 / 13.;
        f[74] = (lo.suit() == trump) as u8 as f32;
        f[75] = trick.is_empty() as u8 as f32;
    }
    f[76] = view.hands.len() as f32 / 6.;
    f[77] = view.trick.num_players() as f32 / 6.;
    f[78] = (!pickup && remaining.in_suit(suit).is_empty()) as u8 as f32;
    f[79] = view.history.in_suit(suit).above(hi).len() as f32 / 13.;
    f
}

pub fn candidates(obs: &[f32]) -> Vec<(usize, [f32; FEATURES])> {
    let Some(view) = from_observation(obs) else {
        return vec![];
    };
    let teacher = action_index(rummy_simple(&view));
    legal_rummy(&view)
        .into_iter()
        .map(|action| (action, features(&view, action, teacher)))
        .collect()
}
