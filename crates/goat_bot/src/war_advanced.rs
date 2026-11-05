//! War policies optimize the eventual rummy game, not the number of tricks won.
//! Search is built solely from a player's view, including public war winnings.
use std::hash::{Hash, Hasher};
use std::time::Instant;

use goat_api::*;
use rand::{Rng, SeedableRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};

use crate::{AdaptSimple, Strategy, learned::*, rummy_simple};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Zero uses AdaptSimple directly; positive values enable full-game search.
    pub samples: usize,
    pub max_rollouts: usize,
    pub max_steps: usize,
    /// Minimum paired reduction in estimated goat probability before changing move.
    pub min_improvement: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            samples: 64,
            max_rollouts: 512,
            max_steps: 4096,
            min_improvement: 0.1,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.samples > 4096
            || self.max_rollouts == 0
            || self.max_rollouts > 65536
            || self.max_steps == 0
            || self.max_steps > 20000
            || !self.min_improvement.is_finite()
            || !(0. ..=1.).contains(&self.min_improvement)
        {
            return Err("invalid war search configuration".into());
        }
        Ok(())
    }
}

pub fn deck_count(war: &War) -> usize {
    (war.deck.cards_remaining()
        + 1
        + war.hands.iter().map(WarHand::len).sum::<usize>()
        + war.won.iter().map(|cards| cards.len()).sum::<usize>()
        + war.trick.cards().count())
        / 52
}

/// Unknown includes the unrevealed trump, private opposing hands, and draw pile.
pub fn unknown_cards(war: &War) -> Cards {
    let mut cards = Cards::ONE_DECK * deck_count(war);
    for won in &war.won {
        cards -= *won;
    }
    cards -= war.trick.cards().collect::<Cards>();
    for hand in &war.hands {
        if let ClientWarHand::Visible(hand) = hand {
            cards -= hand.cards().collect::<Cards>();
        }
    }
    cards
}

pub fn policy(idx: PlayerIdx, war: &War) -> usize {
    AdaptSimple.war(idx, war).map(action_index).unwrap_or(WAIT)
}

/// A sampled real server plus independently redacted clients. No policy sees
/// the sampled server state or the random seed used to construct it.
#[derive(Clone)]
pub struct World {
    pub server: ServerGame,
    pub clients: Vec<ClientGame<(), Cards>>,
}

pub fn sample_world(war: &War, random_seed: u64) -> World {
    let mut rng = rand::rngs::StdRng::seed_from_u64(random_seed);
    let mut unknown: Vec<_> = unknown_cards(war).cards().collect();
    unknown.shuffle(&mut rng);
    let hands: Vec<_> = war
        .hands
        .iter()
        .map(|hand| match hand {
            ClientWarHand::Visible(hand) => hand.clone(),
            ClientWarHand::Hidden(count) => {
                let mut hand = ServerWarHand::new();
                for _ in 0..*count {
                    hand += unknown.pop().expect("war card accounting");
                }
                hand
            }
        })
        .collect();
    assert_eq!(unknown.len(), war.deck.cards_remaining() + 1);
    let players: Vec<_> = (0..hands.len())
        .map(|i| UserId(RandId::from_hash(&[i as u8; 16])))
        .collect();
    let clients = (0..hands.len())
        .map(|actor| {
            let mut view = war.clone();
            view.hands = hands
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    if i == actor {
                        ClientWarHand::Visible(h.clone())
                    } else {
                        ClientWarHand::Hidden(h.len() as u8)
                    }
                })
                .collect();
            ClientGame {
                phase: ClientPhase::War(view),
                players: players.clone(),
            }
        })
        .collect();
    World {
        server: ServerGame {
            phase: ServerPhase::War(WarPhase {
                deck: unknown,
                hands: hands.into_boxed_slice(),
                won: war.won.clone(),
                trick: war.trick.clone(),
                prev_trick: (),
            }),
            players,
            events: Vec::new(),
            seed: rng.r#gen(),
        },
        clients,
    }
}

impl World {
    pub fn apply(&mut self, actor: usize, action: usize) -> Result<(), GoatError> {
        if let Some(action) = decode_action(action) {
            self.server.apply(self.server.players[actor], action)?;
            for event in self.server.events.drain(..) {
                for (i, client) in self.clients.iter_mut().enumerate() {
                    client.apply(event.clone().redact(Some(PlayerIdx(i as u8))))?;
                }
            }
        }
        Ok(())
    }

    fn finish(&mut self, root: usize, max_steps: usize) -> Result<Option<bool>, GoatError> {
        let mut cursor = (root + 1) % self.clients.len();
        for _ in 0..max_steps {
            let (actor, action) = match &self.server.phase {
                ServerPhase::Goat(goat) => return Ok(Some(goat.goat.idx() == root)),
                ServerPhase::War(_) => {
                    let actor = cursor;
                    cursor = (cursor + 1) % self.clients.len();
                    let ClientPhase::War(view) = &self.clients[actor].phase else {
                        unreachable!()
                    };
                    (actor, policy(PlayerIdx(actor as u8), view))
                }
                ServerPhase::Rummy(rummy) => {
                    // Once root has exited it cannot be the goat.
                    if rummy.hands[root].is_empty() {
                        return Ok(Some(false));
                    }
                    let actor = rummy.next.idx();
                    let ClientPhase::Rummy(view) = &self.clients[actor].phase else {
                        unreachable!()
                    };
                    (actor, action_index(rummy_simple(view)))
                }
                _ => unreachable!(),
            };
            self.apply(actor, action)?;
        }
        Ok(None)
    }
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub action: usize,
    pub estimates: Vec<(usize, f32)>,
    pub rollouts: usize,
    pub completed_rounds: usize,
    pub truncated: usize,
    pub elapsed_ms: f64,
}

/// Uses common worlds and adaptive continuations across all root actions. A round with
/// any unfinished outcome is excluded for EVERY action to preserve pairing.
pub fn search(idx: PlayerIdx, war: &War, random_seed: u64, config: &Config) -> SearchResult {
    let start = Instant::now();
    let preferred = policy(idx, war);
    let legal = legal_war(idx, war);
    let mut result = SearchResult {
        action: preferred,
        estimates: Vec::new(),
        rollouts: 0,
        completed_rounds: 0,
        truncated: 0,
        elapsed_ms: 0.,
    };
    if legal.len() <= 1 || config.samples == 0 || deck_count(war) != 1 {
        result.elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
        return result;
    }
    let mut losses = vec![0usize; legal.len()];
    let mut rng = rand::rngs::StdRng::seed_from_u64(random_seed);
    for _ in 0..config.samples.min(config.max_rollouts / legal.len()) {
        let world = sample_world(war, rng.r#gen());
        // Preserve the sampled-world stream of the evaluated policy, which
        // drew one (fixed adaptive) style per player before each next world.
        for _ in 0..war.hands.len() {
            let _ = rng.gen_range(0..1u32);
        }
        let mut outcomes = Vec::with_capacity(legal.len());
        for &action in &legal {
            let mut branch = world.clone();
            branch
                .apply(idx.idx(), action)
                .expect("legal war root action");
            let outcome = branch
                .finish(idx.idx(), config.max_steps)
                .expect("legal war continuation");
            result.rollouts += 1;
            result.truncated += outcome.is_none() as usize;
            outcomes.push(outcome);
        }
        if outcomes.iter().all(Option::is_some) {
            result.completed_rounds += 1;
            for (losses, outcome) in losses.iter_mut().zip(outcomes) {
                *losses += outcome.unwrap() as usize;
            }
        }
    }
    if result.completed_rounds > 0 {
        let preferred_idx = legal
            .iter()
            .position(|&a| a == preferred)
            .expect("legal war baseline");
        let best = (0..legal.len())
            .min_by_key(|&i| (losses[i], legal[i] != preferred))
            .unwrap();
        let gain =
            (losses[preferred_idx] as f32 - losses[best] as f32) / result.completed_rounds as f32;
        if gain > config.min_improvement {
            result.action = legal[best];
        }
        result.estimates = legal
            .iter()
            .zip(losses)
            .map(|(&a, l)| (a, l as f32 / result.completed_rounds as f32))
            .collect();
    }
    result.elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
    result
}

/// Stable for a particular public/private player view, with no server seed.
pub fn seed(idx: PlayerIdx, war: &War) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    idx.idx().hash(&mut hash);
    for feature in observe_war(idx, war) {
        feature.to_bits().hash(&mut hash);
    }
    // Observation truncates long trick order; retain it in the search seed.
    for play in war.trick.plays() {
        (play.player().idx(), play.card, play.kind() as u8).hash(&mut hash);
    }
    hash.finish()
}
