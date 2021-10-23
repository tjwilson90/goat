use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;

use goat_api::{GameId, Response, UserId};

#[derive(Clone)]
pub struct Subscriber {
    pub user_id: UserId,
    pub name: String,
    tx: UnboundedSender<Response>,
    replayed: Option<Arc<Mutex<HashSet<GameId>>>>,
}

impl Subscriber {
    pub fn new(user_id: UserId, name: String, tx: UnboundedSender<Response>) -> Self {
        Self {
            user_id,
            name,
            tx,
            replayed: Some(Arc::new(Mutex::new(HashSet::new()))),
        }
    }

    pub fn send(&mut self, response: Response) -> bool {
        if let Some(replayed) = &mut self.replayed {
            match &response {
                Response::Game { game_id, .. } => {
                    let replayed = replayed.lock();
                    if !replayed.contains(game_id) {
                        return true;
                    }
                }
                Response::Replay { game_id, .. } => {
                    let mut replayed = replayed.lock();
                    replayed.insert(*game_id);
                }
                _ => {}
            }
        }
        log::debug!("Sending {:?} to {}", response, self.user_id);
        self.tx.send(response).is_ok()
    }

    pub fn finish_replay(&mut self) {
        self.replayed = None;
    }
}
