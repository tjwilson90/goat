use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

use goat_api::{Action, Event, GameId, GoatError, Response, ServerGame, UserId};

use crate::Subscriber;

pub struct Server {
    games: RwLock<HashMap<GameId, Mutex<(ServerGame, Instant)>>>,
    subscribers: Mutex<Vec<Subscriber>>,
}

impl Server {
    pub fn new() -> Self {
        Self {
            games: RwLock::new(HashMap::new()),
            subscribers: Mutex::new(Vec::new()),
        }
    }

    pub fn new_game(&self, seed: u64) -> GameId {
        let game_id = GameId(Uuid::new_v4());
        let mut games = self.games.write();
        games.insert(game_id, Mutex::new((ServerGame::new(seed), Instant::now())));
        let mut subscribers = self.subscribers.lock();
        broadcast(
            &mut *subscribers,
            [Response::Replay {
                game_id,
                events: Vec::new(),
            }]
            .iter()
            .cloned(),
        );
        game_id
    }

    pub fn change_name(&self, user_id: UserId, name: String) {
        let mut subscribers = self.subscribers.lock();
        broadcast(
            &mut *subscribers,
            [Response::ChangeName { user_id, name }].iter().cloned(),
        );
    }

    pub fn apply_action(
        &self,
        user_id: UserId,
        game_id: GameId,
        action: Action,
    ) -> Result<(), GoatError> {
        let games = self.games.read();
        let game = match games.get(&game_id) {
            Some(game) => game,
            None => return Err(GoatError::InvalidGame { game_id }),
        };
        let (game, last_updated) = &mut *game.lock();
        let index = game.events.len();
        game.apply(user_id, action)?;
        log::debug!("state {:?}", game);
        *last_updated = Instant::now();
        let mut subscribers = self.subscribers.lock();
        log::debug!(
            "Broadcasting {:?} to {} subs",
            game.events.last(),
            subscribers.len()
        );
        broadcast_events(
            game_id,
            &*game,
            &mut *subscribers,
            game.events[index..].iter().cloned(),
        );
        Ok(())
    }

    pub fn subscribe(&self, user_id: UserId, name: String) -> UnboundedReceiver<Response> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut sub = Subscriber::new(user_id, name.clone(), tx.clone());
        let mut subscribers = self.subscribers.lock();
        broadcast(
            &mut *subscribers,
            [Response::ChangeName { user_id, name }].iter().cloned(),
        );
        subscribers.push(sub.clone());
        for other in &*subscribers {
            sub.send(Response::ChangeName {
                user_id: other.user_id,
                name: other.name.clone(),
            });
        }
        drop(subscribers);
        let games = self.games.read();
        for (game_id, game) in &*games {
            let (game, _) = &*game.lock();
            sub.send(Response::Replay {
                game_id: *game_id,
                events: game.events.clone(),
            });
        }
        sub.finish_replay();
        rx
    }

    pub fn ping_subscribers(&self) {
        let mut subscribers = self.subscribers.lock();
        broadcast(&mut *subscribers, [Response::Ping].iter().cloned());
    }

    pub fn drop_old_games(&self) {
        let mut drops = Vec::new();
        let mut games = self.games.write();
        games.retain(|game_id, game| {
            let (game, last_updated) = &*game.lock();
            let elapsed = last_updated.elapsed();
            let drop = elapsed > Duration::from_secs(18 * 60 * 60)
                || (elapsed > Duration::from_secs(30 * 60)
                    && (!game.started() || game.complete().is_some()));
            if drop {
                drops.push(*game_id);
            }
            !drop
        });
        if !drops.is_empty() {
            let mut subscribers = self.subscribers.lock();
            broadcast(
                &mut *subscribers,
                drops
                    .into_iter()
                    .map(|game_id| Response::Forget { game_id }),
            );
        }
    }
}

fn broadcast(subscribers: &mut Vec<Subscriber>, responses: impl Iterator<Item = Response> + Clone) {
    let mut disconnects = HashSet::new();
    let mut i = 0;
    while i < subscribers.len() {
        let sub = &mut subscribers[i];
        if responses.clone().all(|response| sub.send(response)) {
            i += 1;
        } else {
            disconnects.insert(sub.user_id);
            subscribers.swap_remove(i);
        }
    }
    handle_disconnects(subscribers, disconnects);
}

fn broadcast_events(
    game_id: GameId,
    game: &ServerGame,
    subscribers: &mut Vec<Subscriber>,
    events: impl Iterator<Item = Event> + Clone,
) {
    let mut disconnects = HashSet::new();
    let mut i = 0;
    while i < subscribers.len() {
        let sub = &mut subscribers[i];
        let player = game.player(sub.user_id).ok();
        if events.clone().all(|event| {
            sub.send(Response::Game {
                game_id,
                event: event.redact(player),
            })
        }) {
            i += 1;
        } else {
            disconnects.insert(sub.user_id);
            subscribers.swap_remove(i);
        }
    }
    handle_disconnects(subscribers, disconnects);
}

fn handle_disconnects(subscribers: &mut Vec<Subscriber>, mut disconnects: HashSet<UserId>) {
    if !disconnects.is_empty() {
        for sub in &*subscribers {
            disconnects.remove(&sub.user_id);
        }
    }
    if !disconnects.is_empty() {
        broadcast(
            subscribers,
            disconnects
                .iter()
                .cloned()
                .map(|user_id| Response::Disconnect { user_id }),
        );
    }
}
