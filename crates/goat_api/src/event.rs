use serde::{Deserialize, Serialize};

use crate::{Card, Cards, PlayerIdx, UserId};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum Event {
    #[serde(rename_all = "camelCase")]
    Join { user_id: UserId },
    #[serde(rename_all = "camelCase")]
    Leave { player: PlayerIdx },
    #[serde(rename_all = "camelCase")]
    Start { num_decks: u8 },
    #[serde(rename_all = "camelCase")]
    PlayCard { card: Card },
    #[serde(rename_all = "camelCase")]
    PlayTop { card: Card },
    #[serde(rename_all = "camelCase")]
    Slough { player: PlayerIdx, card: Card },
    #[serde(rename_all = "camelCase")]
    Draw { player: PlayerIdx, card: Card },
    #[serde(rename_all = "camelCase")]
    RevealTrump { trump: Card },
    #[serde(rename_all = "camelCase")]
    OfferDreck { player: PlayerIdx, dreck: Cards },
    #[serde(rename_all = "camelCase")]
    ReceiveDreck { player: PlayerIdx, dreck: Cards },
    #[serde(rename_all = "camelCase")]
    PlayRun { lo: Card, hi: Card },
    #[serde(rename_all = "camelCase")]
    PickUp,
    #[serde(rename_all = "camelCase")]
    RedactedDraw { player: PlayerIdx },
    #[serde(rename_all = "camelCase")]
    RedactedOfferDreck { player: PlayerIdx, dreck: u8 },
    #[serde(rename_all = "camelCase")]
    RedactedReceiveDreck { player: PlayerIdx, dreck: u8 },
}

impl Event {
    pub fn redact(&self, receiver: Option<PlayerIdx>) -> Self {
        use Event::*;
        match self {
            Draw { player, .. } if receiver.unwrap_or(*player) != *player => {
                RedactedDraw { player: *player }
            }
            OfferDreck { player, dreck } if receiver.unwrap_or(*player) != *player => {
                RedactedOfferDreck {
                    player: *player,
                    dreck: dreck.len() as u8,
                }
            }
            ReceiveDreck { player, dreck } if receiver.unwrap_or(*player) != *player => {
                RedactedReceiveDreck {
                    player: *player,
                    dreck: dreck.len() as u8,
                }
            }
            _ => self.clone(),
        }
    }
}
