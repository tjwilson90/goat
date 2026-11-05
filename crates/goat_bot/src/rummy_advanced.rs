//! Selected public-information rollouts and bounded, cached endgame proofs.
use crate::{learned::*, rummy_simple};
use goat_api::*;
use rand::{Rng, SeedableRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{Hash, Hasher},
    time::Instant,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub samples: usize,
    pub max_rollouts: usize,
    pub endgame_cards: usize,
    pub endgame_nodes: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            samples: 16,
            max_rollouts: 1024,
            endgame_cards: 26,
            endgame_nodes: 200_000,
        }
    }
}
impl Config {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.samples == 0
            || self.samples > 4096
            || self.max_rollouts == 0
            || self.max_rollouts > 65536
            || self.endgame_cards > 52
            || self.endgame_nodes > 2_000_000
        {
            return Err("invalid rummy search configuration".into());
        }
        Ok(())
    }
}

/// History includes the current trick; subtracting it twice breaks accounting.
pub fn unknown_cards(view: &Rummy) -> Cards {
    let count = 1 + view.history.len() + view.hands.iter().map(|h| h.len()).sum::<usize>();
    let mut unknown = Cards::ONE_DECK * (count / 52);
    unknown -= view.trump;
    unknown -= view.history;
    for hand in &view.hands {
        unknown -= hand.known;
    }
    unknown
}

#[derive(Clone, Debug)]
pub struct Result {
    pub action: usize,
    pub estimates: Vec<(usize, f32, usize)>,
    pub rollouts: usize,
    pub truncated: usize,
    pub exact: bool,
    pub nodes: usize,
    pub elapsed_ms: f64,
}

/// Only public ownership is shared. Each player sees its own sampled private hand.
pub fn sample_views(view: &Rummy, seed: u64) -> Vec<Rummy> {
    let mut unknown: Vec<_> = unknown_cards(view).cards().collect();
    unknown.shuffle(&mut rand::rngs::StdRng::seed_from_u64(seed));
    let mut cards = unknown.into_iter();
    let mut full = view.clone();
    for hand in &mut full.hands {
        for _ in 0..hand.unknown {
            hand.known += cards.next().expect("card accounting");
        }
        hand.unknown = 0;
    }
    assert!(cards.next().is_none());
    (0..view.hands.len())
        .map(|actor| {
            let mut result = view.clone();
            for (i, hand) in result.hands.iter_mut().enumerate() {
                hand.known = if i == actor {
                    full.hands[i].known
                } else {
                    hand.public
                };
                hand.unknown = (full.hands[i].len() - hand.known.len()) as u8;
            }
            result
        })
        .collect()
}

fn advance(view: &mut Rummy, action: usize) -> Option<usize> {
    let actor = view.next;
    match decode_action(action).unwrap() {
        Action::PlayRun { lo, hi } => {
            view.play_run(actor, lo, hi).expect("legal simulated play");
            view.is_finished().then_some(view.next.idx())
        }
        Action::PickUp => view
            .pick_up(actor)
            .expect("legal simulated pickup")
            .then_some(actor.idx()),
        _ => unreachable!(),
    }
}
fn state_key(view: &Rummy, root: usize, information: bool) -> Vec<u128> {
    let mut result = vec![
        root as u128,
        view.trump as u128,
        view.hands.len() as u128,
        view.next.0 as u128,
        view.pick_ups,
        view.trick.num_players() as u128,
    ];
    if information {
        result.push(view.history.bits);
    }
    for hand in &view.hands {
        result.push(hand.known.bits);
        if information {
            result.extend([hand.unknown as u128, hand.public.bits]);
        }
    }
    result.extend(view.trick.plays().iter().map(|p| {
        ((p.player().0 as u128) << 32)
            | action_index(Action::PlayRun {
                lo: p.lo(),
                hi: p.hi(),
            }) as u128
    }));
    result
}
thread_local! { static PROOFS: RefCell<HashMap<Vec<u128>, bool>> = RefCell::new(HashMap::new()); }

/// Isolate independent benchmark games. Cached proofs are sound, but warming
/// the cache can change which additional proofs fit a bounded node budget.
pub fn clear_proof_cache() {
    PROOFS.with(|cache| cache.borrow_mut().clear());
}

fn ordered(view: &Rummy, preferred: usize) -> Vec<usize> {
    let mut actions = legal_rummy(view);
    let hand = view.hands[view.next.idx()].len();
    actions.sort_by_key(|&a| {
        let count = match decode_action(a).unwrap() {
            Action::PlayRun { lo, hi } => Cards::range(lo, hi).len(),
            _ => 0,
        };
        (count != hand, a != preferred, a == 0, usize::MAX - count, a)
    });
    actions
}
fn solve(
    view: &Rummy,
    root: usize,
    left: &mut usize,
    depth: usize,
    memo: &mut HashMap<Vec<u128>, bool>,
) -> Option<bool> {
    if view.hands[root].is_empty() {
        return Some(true);
    }
    if view.is_finished() {
        return Some(view.next.idx() != root);
    }
    if *left == 0 || depth == 0 {
        return None;
    }
    let key = state_key(view, root, false);
    if let Some(&result) = memo.get(&key) {
        return Some(result);
    }
    *left -= 1;
    let desired = view.next.idx() == root;
    let mut unresolved = false;
    for a in ordered(view, action_index(rummy_simple(view))) {
        let mut next = view.clone();
        let result = if let Some(goat) = advance(&mut next, a) {
            Some(goat != root)
        } else {
            solve(&next, root, left, depth - 1, memo)
        };
        match result {
            Some(v) if v == desired => {
                memo.insert(key, v);
                return Some(v);
            }
            None => unresolved = true,
            _ => (),
        }
    }
    if unresolved {
        None
    } else {
        memo.insert(key, !desired);
        Some(!desired)
    }
}
fn prove(view: &Rummy, preferred: usize, config: &Config) -> (Option<usize>, usize) {
    if view.hands.iter().filter(|h| !h.is_empty()).count() != 2
        || view.hands.iter().map(|h| h.len()).sum::<usize>() > config.endgame_cards
    {
        return (None, 0);
    }
    let mut full = view.clone();
    let unknown: Vec<_> = full
        .hands
        .iter()
        .enumerate()
        .filter(|(_, h)| h.unknown > 0)
        .map(|(i, _)| i)
        .collect();
    if unknown.len() == 1 {
        let cards = unknown_cards(&full);
        full.hands[unknown[0]].known += cards;
        full.hands[unknown[0]].unknown = 0;
    } else if !unknown.is_empty() {
        return (None, 0);
    }
    PROOFS.with(|cache| {
        let mut memo = cache.borrow_mut();
        if memo.len() > 100_000 {
            memo.clear();
        }
        let root = full.next.idx();
        let mut used = 0;
        let actions = ordered(&full, preferred);
        let allowance = config.endgame_nodes / actions.len().max(1);
        for a in actions {
            let mut child = full.clone();
            let mut left = allowance;
            let result = if let Some(goat) = advance(&mut child, a) {
                Some(goat != root)
            } else {
                solve(&child, root, &mut left, 256, &mut memo)
            };
            used += allowance - left;
            if result == Some(true) {
                return (Some(a), used);
            }
        }
        (None, used)
    })
}

/// A proved action cannot lose against any two-player continuation. A depleted
/// node budget supplies no conclusion.
pub fn proved_safe_action(view: &Rummy, preferred: usize, nodes: usize) -> Option<usize> {
    prove(
        view,
        preferred,
        &Config {
            endgame_nodes: nodes,
            ..Default::default()
        },
    )
    .0
}

fn rollout(mut views: Vec<Rummy>, first: usize, root: usize) -> Option<usize> {
    let mut action = first;
    for _ in 0..2048 {
        let mut goat = None;
        for view in &mut views {
            goat = advance(view, action);
        }
        if let Some(goat) = goat {
            return Some(goat);
        }
        if views[root].hands[root].is_empty() {
            return Some(usize::MAX);
        }
        let actor = views[0].next.idx();
        action = action_index(rummy_simple(&views[actor]));
    }
    None
}

pub fn search(view: &Rummy, seed: u64, preferred: usize, cfg: &Config) -> Result {
    let start = Instant::now();
    let legal = ordered(view, preferred);
    let preferred = if legal.contains(&preferred) {
        preferred
    } else {
        legal[0]
    };
    if legal.len() == 1 {
        return Result {
            action: legal[0],
            estimates: vec![],
            rollouts: 0,
            truncated: 0,
            exact: false,
            nodes: 0,
            elapsed_ms: 0.,
        };
    }
    let (safe, nodes) = prove(view, preferred, cfg);
    if let Some(a) = safe {
        return Result {
            action: a,
            estimates: vec![(a, 0., 1)],
            rollouts: 0,
            truncated: 0,
            exact: true,
            nodes,
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.,
        };
    }
    let root = view.next.idx();
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut counts = vec![(0usize, 0usize); legal.len()];
    let mut total = 0;
    let mut truncated = 0;
    // With at most one unknown hand, card accounting determines the entire
    // world. Repeating a deterministic continuation adds no information.
    let deterministic = view.hands.iter().filter(|hand| hand.unknown > 0).count() <= 1;
    let rounds = if deterministic { 1 } else { cfg.samples };
    for round in 0..rounds {
        if total >= cfg.max_rollouts {
            break;
        }
        let sample_seed = rng.r#gen();
        let views = sample_views(view, sample_seed);
        // Retain the random stream of the evaluated policy: its fixed simple
        // continuation consumed one usize style draw per player and world.
        for _ in 0..view.hands.len() {
            let _ = rng.gen_range(0..1usize);
        }
        for offset in 0..legal.len() {
            let i = (offset + round) % legal.len();
            let a = legal[i];
            if total >= cfg.max_rollouts {
                break;
            }
            total += 1;
            if let Some(goat) = rollout(views.clone(), a, root) {
                counts[i].0 += (goat == root) as usize;
                counts[i].1 += 1;
            } else {
                truncated += 1;
            }
        }
    }
    let estimates: Vec<_> = legal
        .iter()
        .zip(&counts)
        .filter(|(_, (_, n))| *n > 0)
        .map(|(&a, &(l, n))| (a, l as f32 / n as f32, n))
        .collect();
    let action = if counts.iter().any(|(_, n)| *n == 0) {
        preferred
    } else {
        estimates
            .iter()
            .min_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| (a.0 != preferred).cmp(&(b.0 != preferred)))
                    .then(a.0.cmp(&b.0))
            })
            .map_or(preferred, |r| r.0)
    };
    Result {
        action,
        estimates,
        rollouts: total,
        truncated,
        exact: false,
        nodes,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.,
    }
}

pub fn seed(view: &Rummy) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    state_key(view, view.next.idx(), true).hash(&mut hash);
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ending() -> Rummy {
        let mut view = Rummy::new(
            vec![
                ClientRummyHand {
                    known: Cards::from(Card::TwoClubs),
                    unknown: 0,
                    public: Cards::NONE,
                },
                ClientRummyHand {
                    known: Cards::from(Card::ThreeClubs),
                    unknown: 0,
                    public: Cards::NONE,
                },
            ]
            .into_boxed_slice(),
            PlayerIdx(0),
            Card::AceSpades,
        );
        view.history = Cards::ONE_DECK - Card::TwoClubs - Card::ThreeClubs - Card::AceSpades;
        view
    }

    #[test]
    fn proofs_cover_exits_and_do_not_cache_budget_exhaustion() {
        let view = ending();
        let preferred = action_index(rummy_simple(&view));
        let cfg = Config {
            endgame_nodes: 0,
            ..Default::default()
        };
        assert_eq!(prove(&view, preferred, &cfg).0, Some(preferred));
        let mut blocked = view.clone();
        blocked
            .trick
            .play(PlayerIdx(1), Card::AceClubs, Card::AceClubs);
        let mut memo = HashMap::new();
        assert_eq!(solve(&blocked, 0, &mut 0, 256, &mut memo), None);
        assert!(memo.is_empty());
        blocked.pick_ups = 63 | (64 << 8);
        assert_eq!(solve(&blocked, 0, &mut 100, 256, &mut memo), Some(false));
    }

    #[test]
    fn information_keys_include_history_and_proof_keys_include_rule_state() {
        let view = ending();
        let key = state_key(&view, 0, true);
        let mut other = view.clone();
        other.history -= Card::AceClubs;
        assert_ne!(key, state_key(&other, 0, true));
        other = view.clone();
        other.pick_ups = 1;
        assert_ne!(state_key(&view, 0, false), state_key(&other, 0, false));
        other = view.clone();
        other.trump = Card::AceHearts;
        assert_ne!(state_key(&view, 0, false), state_key(&other, 0, false));
        assert_ne!(state_key(&view, 0, false), state_key(&view, 1, false));
    }

    #[test]
    fn deterministic_worlds_are_not_resampled() {
        let mut view = ending();
        view.hands[0].known += Card::FourClubs;
        view.history -= Card::FourClubs;
        let preferred = action_index(rummy_simple(&view));
        let mut cfg = Config {
            samples: 64,
            endgame_cards: 0,
            ..Default::default()
        };
        let a = search(&view, 42, preferred, &cfg);
        cfg.samples = 1;
        let b = search(&view, 42, preferred, &cfg);
        assert_eq!(a.estimates, b.estimates);
        assert_eq!(a.action, b.action);
        assert_eq!(a.rollouts, legal_rummy(&view).len());
    }
}
