//! Versioned observation/action schema shared by training and the selected bot.
use goat_api::*;

pub type War = WarPhase<ClientDeck, ClientWarHand, ()>;
pub type Rummy = RummyPhase<ClientRummyHand, Cards>;
pub const MAX_PLAYERS: usize = 15;
pub const RUMMY_ACTIONS: usize = 365;
pub const DRAW: usize = 469;
pub const PLAY_TOP: usize = 470;
pub const FINISH: usize = 471;
pub const WAIT: usize = 472;
pub const ACTIONS: usize = 473;
pub const SEAT_FEATURES: usize = 111;
pub const OBSERVATIONS: usize = MAX_PLAYERS * SEAT_FEATURES + 16 + 5 * 52 + 15 * 8;
pub const SCHEMA: u32 = 1;

pub fn card_index(card: Card) -> usize {
    13 * card.suit().idx() + card.rank().idx()
}

fn card_at(index: usize) -> Card {
    Card::new(
        Rank::from((index % 13) as u8),
        Suit::from((index / 13) as u8),
    )
}

pub fn action_index(action: Action) -> usize {
    match action {
        Action::PickUp => 0,
        Action::PlayRun { lo, hi } => {
            let low = lo.rank().idx();
            1 + lo.suit().idx() * 91 + low * 13 - low * low.saturating_sub(1) / 2 + hi.rank().idx()
                - low
        }
        Action::PlayCard { card } => 365 + card_index(card),
        Action::Slough { card } => 417 + card_index(card),
        Action::Draw => DRAW,
        Action::PlayTop => PLAY_TOP,
        Action::FinishTrick => FINISH,
        _ => panic!("not a policy action: {action:?}"),
    }
}

/// WAIT is a scheduler operation, never a server action.
pub fn decode_action(index: usize) -> Option<Action> {
    match index {
        0 => Some(Action::PickUp),
        1..365 => {
            let suit = (index - 1) / 91;
            let mut offset = (index - 1) % 91;
            for low in 0..13 {
                if offset < 13 - low {
                    return Some(Action::PlayRun {
                        lo: card_at(suit * 13 + low),
                        hi: card_at(suit * 13 + low + offset),
                    });
                }
                offset -= 13 - low;
            }
            unreachable!()
        }
        365..417 => Some(Action::PlayCard {
            card: card_at(index - 365),
        }),
        417..469 => Some(Action::Slough {
            card: card_at(index - 417),
        }),
        DRAW => Some(Action::Draw),
        PLAY_TOP => Some(Action::PlayTop),
        FINISH => Some(Action::FinishTrick),
        _ => None,
    }
}

pub fn legal_rummy(rummy: &Rummy) -> Vec<usize> {
    let hand = rummy.hands[rummy.next.idx()].known;
    let mut legal = Vec::new();
    if !rummy.trick.is_empty() {
        legal.push(0);
    }
    for suit in Suit::VALUES {
        for low in 0..13 {
            let lo = Card::new(Rank::from(low), suit);
            if !hand.contains(lo) || !rummy.trick.can_play(lo, rummy.trump.suit()) {
                continue;
            }
            for high in low..13 {
                let hi = Card::new(Rank::from(high), suit);
                if !hand.contains(hi) {
                    break;
                }
                legal.push(action_index(Action::PlayRun { lo, hi }));
            }
        }
    }
    legal
}

pub fn legal_war(idx: PlayerIdx, war: &War) -> Vec<usize> {
    let ClientWarHand::Visible(hand) = &war.hands[idx.idx()] else {
        return vec![];
    };
    let mut legal = Vec::new();
    if hand.len() < 3 && war.deck.cards_remaining() > 0 {
        legal.push(DRAW);
    }
    let finished = war.is_finished() || war.trick.winner().is_some();
    for card in hand.cards() {
        if war.trick.check_can_slough(idx, hand, card).is_ok() {
            legal.push(action_index(Action::Slough { card }));
        }
        if !finished && war.trick.check_can_play(idx, hand, card).is_ok() {
            legal.push(action_index(Action::PlayCard { card }));
        }
    }
    if !finished
        && war.deck.cards_remaining() > 0
        && war.trick.check_can_play_top(idx, hand).is_ok()
    {
        legal.push(PLAY_TOP);
    }
    if finished && !war.trick.ended(idx) {
        legal.push(FINISH);
    } else if finished || war.trick.next_player() != Some(idx) {
        // Mandatory plays/acknowledgments cannot wait indefinitely.
        legal.push(WAIT);
    }
    legal.sort_unstable();
    legal.dedup();
    legal
}

fn encode_cards(out: &mut [f32], cards: Cards) {
    for card in cards.cards() {
        out[card_index(card)] += 1.0 / 3.0;
    }
}

pub fn observe_war(idx: PlayerIdx, war: &War) -> Vec<f32> {
    let mut out = vec![0.; OBSERVATIONS];
    let n = war.hands.len();
    for relative in 0..n {
        let player = (idx.idx() + relative) % n;
        let row = &mut out[relative * SEAT_FEATURES..(relative + 1) * SEAT_FEATURES];
        if let ClientWarHand::Visible(hand) = &war.hands[player] {
            encode_cards(row, hand.cards().collect());
        }
        encode_cards(&mut row[52..], war.won[player]);
        row[104] = war.hands[player].len() as f32 / 156.;
        row[105] = if let ClientWarHand::Hidden(count) = war.hands[player] {
            count as f32 / 156.
        } else {
            0.
        };
        row[106] = 1.;
        row[107] = war.trick.remaining_players().any(|p| p.idx() == player) as u8 as f32;
        row[108] = war.trick.ended(PlayerIdx(player as u8)) as u8 as f32;
        row[110] = 1.;
    }
    let global = &mut out[MAX_PLAYERS * SEAT_FEATURES..];
    global[0] = 1.;
    global[2] = n as f32 / 15.;
    global[3] = war.deck.cards_remaining() as f32 / 156.;
    global[4] = war
        .trick
        .rank()
        .map_or(0., |rank| (rank.idx() + 1) as f32 / 13.);
    global[5] = war
        .trick
        .next_player()
        .map_or(0., |p| ((p.idx() + n - idx.idx()) % n + 1) as f32 / 15.);
    global[6] = war.trick.winner().is_some() as u8 as f32;
    global[7] = war.is_finished() as u8 as f32;
    encode_cards(&mut global[16..], war.trick.cards().collect());
    // Keep recent order as well as aggregate cards; long tie histories are summarized.
    for (i, play) in war.trick.plays().iter().rev().take(15).enumerate() {
        let row = &mut global[276 + i * 8..];
        row[0] = (play.card.rank().idx() + 1) as f32 / 13.;
        row[1] = play.kind() as u8 as f32 / 2.;
        row[2 + play.card.suit().idx()] = 1.;
        row[6] = ((play.player().idx() + n - idx.idx()) % n) as f32 / 15.;
        row[7] = 1.;
    }
    out
}

pub fn observe_rummy(rummy: &Rummy) -> Vec<f32> {
    let mut out = vec![0.; OBSERVATIONS];
    let n = rummy.hands.len();
    let idx = rummy.next.idx();
    for relative in 0..n {
        let player = (idx + relative) % n;
        let hand = &rummy.hands[player];
        let row = &mut out[relative * SEAT_FEATURES..(relative + 1) * SEAT_FEATURES];
        encode_cards(row, hand.known);
        row[104] = hand.len() as f32 / 156.;
        row[105] = hand.unknown as f32 / 156.;
        row[106] = !hand.is_empty() as u8 as f32;
        row[109] = ((rummy.pick_ups >> (8 * player)) & 255) as f32 / 64.;
        row[110] = 1.;
    }
    let global = &mut out[MAX_PLAYERS * SEAT_FEATURES..];
    global[1] = 1.;
    global[2] = n as f32 / 15.;
    global[8] = rummy.trick.num_players() as f32 / 15.;
    global[9] = rummy.trick.len() as f32 / 15.;
    global[10] = (1 + rummy.history.len() + rummy.hands.iter().map(|h| h.len()).sum::<usize>())
        as f32
        / 156.;
    encode_cards(&mut global[16..], rummy.history);
    encode_cards(&mut global[68..], Cards::from(rummy.trump));
    if let Some(card) = rummy.trick.top_card() {
        encode_cards(&mut global[120..], Cards::from(card));
        let mut trick = rummy.trick.clone();
        let (lo, hi) = trick.pick_up();
        encode_cards(&mut global[172..], Cards::range(lo, hi));
    }
    for (i, play) in rummy.trick.plays().iter().enumerate() {
        encode_cards(&mut global[224..], Cards::range(play.lo(), play.hi()));
        let row = &mut global[276 + i * 8..];
        row[0] = (play.lo().rank().idx() + 1) as f32 / 13.;
        row[1] = (play.hi().rank().idx() + 1) as f32 / 13.;
        row[2 + play.lo().suit().idx()] = 1.;
        row[6] = ((play.player().idx() + n - idx) % n) as f32 / 15.;
        row[7] = 1.;
    }
    out
}
