use serde::{Deserialize, Serialize};

use crate::{Card, Cards, PlayerIdx, UserId};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Join { user_id: UserId },
    Leave { player: PlayerIdx },
    Start { num_decks: u8 },
    PlayCard { card: Card },
    PlayTop { card: Card },
    Slough { player: PlayerIdx, card: Card },
    Draw { player: PlayerIdx, card: Card },
    RevealTrump { trump: Card },
    OfferDreck { player: PlayerIdx, dreck: Cards },
    ReceiveDreck { player: PlayerIdx, dreck: Cards },
    PlayRun { lo: Card, hi: Card },
    PickUp,
    RedactedDraw { player: PlayerIdx },
    RedactedOfferDreck { player: PlayerIdx, dreck: u8 },
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
