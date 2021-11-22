use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};
use smallvec::SmallVec;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

use goat_api::{Action, Event, GameId, GoatError, Response, ServerGame, User, UserId};

use crate::Subscriber;

pub struct Server {
    games: RwLock<HashMap<GameId, Mutex<(ServerGame, Instant)>>>,
    users: Mutex<HashMap<UserId, ServerUser>>,
}

struct ServerUser {
    name: String,
    subs: SmallVec<[Subscriber; 1]>,
}

impl Server {
    pub fn new() -> Self {
        Self {
            games: RwLock::new(HashMap::new()),
            users: Mutex::new(HashMap::new()),
        }
    }

    pub fn new_game(&self, seed: u64) -> GameId {
        let game_id = GameId(Uuid::new_v4());
        let mut games = self.games.write();
        games.insert(game_id, Mutex::new((ServerGame::new(seed), Instant::now())));
        let mut users = self.users.lock();
        broadcast(
            &mut *users,
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
        let mut users = self.users.lock();
        let result = match users.entry(user_id) {
            Entry::Occupied(e) => {
                let user = e.into_mut();
                if user.name != name {
                    user.name = name.clone();
                    Some(!user.subs.is_empty())
                } else {
                    None
                }
            }
            Entry::Vacant(e) => {
                e.insert(ServerUser {
                    name: name.clone(),
                    subs: SmallVec::new(),
                });
                Some(false)
            }
        };
        if let Some(online) = result {
            broadcast(
                &mut *users,
                [Response::User {
                    user_id,
                    user: User { name, online },
                }]
                .iter()
                .cloned(),
            );
        }
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
        let mut users = self.users.lock();
        broadcast_events(
            game_id,
            &*game,
            &mut *users,
            game.events[index..].iter().cloned(),
        );
        Ok(())
    }

    pub fn subscribe(&self, user_id: UserId, name: String) -> UnboundedReceiver<Response> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut sub = Subscriber::new(tx);

        let mut users = self.users.lock();
        for (&user_id, user) in users.iter() {
            sub.send(Response::User {
                user_id,
                user: User {
                    name: user.name.clone(),
                    online: !user.subs.is_empty(),
                },
            });
        }
        let user = users.entry(user_id).or_insert_with(|| ServerUser {
            name: String::new(),
            subs: SmallVec::new(),
        });
        user.subs.push(sub.clone());
        if name != user.name || user.subs.len() == 1 {
            user.name = name.clone();
            broadcast(
                &mut *users,
                [Response::User {
                    user_id,
                    user: User { name, online: true },
                }]
                .iter()
                .cloned(),
            );
        }
        drop(users);

        let games = self.games.read();
        for (game_id, game) in &*games {
            let (game, _) = &*game.lock();
            let player = game.player(user_id).ok();
            sub.send(Response::Replay {
                game_id: *game_id,
                events: game.events.iter().map(|e| e.redact(player)).collect(),
            });
        }
        sub.finish_replay();
        rx
    }

    pub fn ping_subscribers(&self) {
        let mut users = self.users.lock();
        broadcast(&mut *users, [Response::Ping].iter().cloned());
    }

    pub fn forget_old_state(&self, max_age: Duration, complete_age: Duration) {
        let mut players = HashSet::new();
        let mut drops = Vec::new();
        let mut games = self.games.write();
        games.retain(|game_id, game| {
            let (game, last_updated) = &*game.lock();
            let elapsed = last_updated.elapsed();
            let drop = elapsed > max_age || (elapsed > complete_age && !game.active());
            if drop {
                drops.push(*game_id);
            } else {
                for user_id in &game.players {
                    players.insert(*user_id);
                }
            }
            !drop
        });
        let mut users = self.users.lock();
        if !drops.is_empty() {
            broadcast(
                &mut *users,
                drops
                    .into_iter()
                    .map(|game_id| Response::ForgetGame { game_id }),
            );
        }
        drop(games);
        let mut drops = Vec::new();
        users.retain(|user_id, user| {
            let drop = user.subs.is_empty() && !players.contains(user_id);
            if drop {
                drops.push(*user_id);
            }
            !drop
        });
        if !drops.is_empty() {
            broadcast(
                &mut *users,
                drops
                    .into_iter()
                    .map(|user_id| Response::ForgetUser { user_id }),
            );
        }
    }
}

fn broadcast(
    users: &mut HashMap<UserId, ServerUser>,
    responses: impl Iterator<Item = Response> + Clone,
) {
    let mut disconnects = Vec::new();
    for (&user_id, user) in users.iter_mut().filter(|(_, user)| !user.subs.is_empty()) {
        let mut i = 0;
        while i < user.subs.len() {
            let sub = &mut user.subs[i];
            if responses.clone().all(|response| sub.send(response)) {
                i += 1;
            } else {
                user.subs.swap_remove(i);
            }
        }
        if user.subs.is_empty() {
            disconnects.push(Response::User {
                user_id,
                user: User {
                    name: user.name.clone(),
                    online: false,
                },
            });
        }
    }
    if !disconnects.is_empty() {
        broadcast(users, disconnects.into_iter());
    }
}

fn broadcast_events(
    game_id: GameId,
    game: &ServerGame,
    users: &mut HashMap<UserId, ServerUser>,
    events: impl Iterator<Item = Event> + Clone,
) {
    let mut disconnects = Vec::new();
    for (&user_id, user) in users.iter_mut().filter(|(_, user)| !user.subs.is_empty()) {
        let mut i = 0;
        while i < user.subs.len() {
            let sub = &mut user.subs[i];
            let player = game.player(user_id).ok();
            if events.clone().all(|event| {
                sub.send(Response::Game {
                    game_id,
                    event: event.redact(player),
                })
            }) {
                i += 1;
            } else {
                user.subs.swap_remove(i);
            }
        }
        if user.subs.is_empty() {
            disconnects.push(Response::User {
                user_id,
                user: User {
                    name: user.name.clone(),
                    online: false,
                },
            });
        }
    }
    if !disconnects.is_empty() {
        broadcast(users, disconnects.into_iter());
    }
}
