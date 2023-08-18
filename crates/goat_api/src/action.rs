use serde::Deserialize;

use crate::{Card, PlayerIdx, UserId};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum Action {
    #[serde(rename_all = "camelCase")]
    Join { user_id: UserId },
    #[serde(rename_all = "camelCase")]
    Leave { player: PlayerIdx },
    #[serde(rename_all = "camelCase")]
    Start { num_decks: u8 },
    #[serde(rename_all = "camelCase")]
    PlayCard { card: Card },
    #[serde(rename_all = "camelCase")]
    PlayTop,
    #[serde(rename_all = "camelCase")]
    Slough { card: Card },
    #[serde(rename_all = "camelCase")]
    Draw,
    #[serde(rename_all = "camelCase")]
    FinishTrick,
    #[serde(rename_all = "camelCase")]
    PlayRun { lo: Card, hi: Card },
    #[serde(rename_all = "camelCase")]
    PickUp,
    #[serde(rename_all = "camelCase")]
    Goat { noise: usize },
}
