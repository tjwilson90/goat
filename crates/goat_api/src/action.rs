use serde::Deserialize;

use crate::{Card, PlayerIdx, UserId};

#[derive(Clone, Copy, Debug, Deserialize)]
pub enum Action {
    Join { user_id: UserId },
    Leave { player: PlayerIdx },
    Start { num_decks: u8 },
    PlayCard { card: Card },
    PlayTop,
    Slough { card: Card },
    Draw,
    PlayRun { lo: Card, hi: Card },
    PickUp,
}
