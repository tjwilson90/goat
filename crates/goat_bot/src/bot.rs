use std::collections::hash_map::Entry;
use std::collections::HashSet;

use tokio::sync::mpsc::UnboundedReceiver;

use goat_api::{Action, Client, ClientPhase, GameId, GoatError, PlayerIdx, Response, UserId};

use crate::Strategy;

pub struct Bot<Tx, S> {
    client: Client<()>,
    user_id: UserId,
    rx: UnboundedReceiver<Response>,
    tx: Tx,
    strategy: S,
}

impl<Tx: Fn(UserId, GameId, Action) -> Result<(), GoatError>, S: Strategy> Bot<Tx, S> {
    pub fn new(user_id: UserId, rx: UnboundedReceiver<Response>, tx: Tx, strategy: S) -> Self {
        Self {
            client: Client::new(()),
            user_id,
            rx,
            tx,
            strategy,
        }
    }

    pub async fn run(&mut self) -> Result<(), GoatError> {
        let mut changed = HashSet::new();
        loop {
            match self.rx.recv().await {
                Some(response) => {
                    changed_game(&response).map(|id| changed.insert(id));
                    log::debug!("recv {}: {:?}", self.user_id, response);
                    self.client.apply(response)?;
                    log::debug!("state {}: {:?}", self.user_id, self.client.games);
                }
                None => return Ok(()),
            };
            while let Ok(response) = self.rx.try_recv() {
                changed_game(&response).map(|id| changed.insert(id));
                log::debug!("try_recv {}: {:?}", self.user_id, response);
                self.client.apply(response)?;
                log::debug!("state {}: {:?}", self.user_id, self.client.games);
            }
            for game_id in changed.drain() {
                if let Some(action) = self.action(game_id) {
                    if let Err(e) = (self.tx)(self.user_id, game_id, action) {
                        log::warn!(
                            "Bot {} failed. state={:?}, action={:?}, error={:?}",
                            self.user_id,
                            self.client.games.get(&game_id),
                            action,
                            e
                        );
                    }
                } else if let Entry::Occupied(game) = self.client.games.entry(game_id) {
                    if matches!(game.get().phase, ClientPhase::Complete(_)) {
                        game.remove();
                    }
                }
            }
        }
    }

    fn action(&self, game_id: GameId) -> Option<Action> {
        let game = self.client.games.get(&game_id).unwrap();
        let idx = game.players.iter().position(|id| *id == self.user_id)?;
        let idx = PlayerIdx(idx as u8);
        match &game.phase {
            ClientPhase::Unstarted | ClientPhase::Complete(_) => None,
            ClientPhase::War(war) => self.strategy.war(idx, war),
            ClientPhase::Rummy(rummy) => {
                if rummy.next == idx {
                    Some(self.strategy.rummy(idx, rummy))
                } else {
                    None
                }
            }
        }
    }
}

fn changed_game(response: &Response) -> Option<GameId> {
    match response {
        Response::Replay { game_id, .. } | Response::Game { game_id, .. } => Some(*game_id),
        _ => None,
    }
}
