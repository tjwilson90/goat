use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;

use goat_api::{GameId, Response};

#[derive(Clone)]
pub struct Subscriber {
    tx: UnboundedSender<Response>,
    replayed: Option<Arc<Mutex<HashSet<GameId>>>>,
}

impl Subscriber {
    pub fn new(tx: UnboundedSender<Response>) -> Self {
        Self {
            tx,
            replayed: Some(Arc::new(Mutex::new(HashSet::new()))),
        }
    }

    pub fn disconnected() -> Self {
        let (tx, _) = mpsc::unbounded_channel();
        Self { tx, replayed: None }
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
        log::debug!("Sending {:?}", response);
        self.tx.send(response).is_ok()
    }

    pub fn online(&self) -> bool {
        !self.tx.is_closed()
    }

    pub fn finish_replay(&mut self) {
        self.replayed = None;
    }
}
