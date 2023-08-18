use std::collections::HashSet;

use rand::Rng;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{Duration, Instant};

use goat_api::{
    Action, Cards, Client, ClientPhase, GameId, GoatError, PlayerIdx, Response, UserId,
};

use crate::Strategy;

pub struct Bot<Tx, S> {
    client: Client<(), (), Cards>,
    user_id: UserId,
    rx: UnboundedReceiver<Response>,
    tx: Tx,
    strategy: S,
    sleep: fn(Action) -> Duration,
}

impl<
        Tx: Fn(UserId, GameId, Action) -> Result<(), GoatError> + Clone + Send + Sync + 'static,
        S: Strategy,
    > Bot<Tx, S>
{
    pub fn new(
        user_id: UserId,
        rx: UnboundedReceiver<Response>,
        tx: Tx,
        strategy: S,
        sleep: fn(Action) -> Duration,
    ) -> Self {
        Self {
            client: Client::new(()),
            user_id,
            rx,
            tx,
            strategy,
            sleep,
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
                let start = Instant::now();
                if let Some(action) = self.action(game_id).await {
                    let mut duration = (self.sleep)(action);
                    duration = duration.saturating_sub(start.elapsed());
                    if duration == Duration::ZERO {
                        let _ = (self.tx)(self.user_id, game_id, action);
                    } else {
                        let tx = self.tx.clone();
                        let user_id = self.user_id;
                        tokio::spawn(async move {
                            tokio::time::sleep(duration).await;
                            let _ = tx(user_id, game_id, action);
                        });
                    }
                }
            }
        }
    }

    async fn action(&self, game_id: GameId) -> Option<Action> {
        let game = self.client.games.get(&game_id)?;
        let idx = game.players.iter().position(|id| *id == self.user_id)?;
        let idx = PlayerIdx(idx as u8);
        match &game.phase {
            ClientPhase::Unstarted => None,
            ClientPhase::War(war) => self.strategy.war(idx, war),
            ClientPhase::Rummy(rummy) => {
                if rummy.next == idx {
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    rayon::scope(|scope| {
                        scope.spawn(|_| {
                            tx.send(self.strategy.rummy(rummy)).unwrap();
                        });
                    });
                    Some(rx.await.unwrap())
                } else {
                    None
                }
            }
            ClientPhase::Goat(goat) => {
                if goat.goat == idx && goat.noise.is_none() {
                    let total_noises = std::fs::read_dir("./assets/noises/").ok()?.count();
                    let noise = rand::thread_rng().gen_range(0..total_noises);
                    Some(Action::Goat { noise })
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
