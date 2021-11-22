use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};

use goat_api::{Card, PlayerIdx, RummyHistory};

pub struct OneAction {
    history: Box<[(Card, Card)]>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum LastAction {
    Play { lo: Card, hi: Card },
    PickUp,
    None,
}

const NONE_SENTINEL: (Card, Card) = (Card::TwoDiamonds, Card::TwoSpades);
const PICK_UP_SENTINEL: (Card, Card) = (Card::TwoClubs, Card::TwoDiamonds);

impl OneAction {
    pub fn last_action(&self, player: PlayerIdx) -> LastAction {
        let action = self.history[player.idx()];
        if action == NONE_SENTINEL {
            LastAction::None
        } else if action == PICK_UP_SENTINEL {
            LastAction::PickUp
        } else {
            LastAction::Play {
                lo: action.0,
                hi: action.1,
            }
        }
    }
}

impl RummyHistory for OneAction {
    fn new(num_players: usize) -> Self {
        Self {
            history: vec![NONE_SENTINEL; num_players].into_boxed_slice(),
        }
    }

    fn play(&mut self, player: PlayerIdx, lo: Card, hi: Card) {
        self.history[player.idx()] = (lo, hi);
    }

    fn pick_up(&mut self, player: PlayerIdx) {
        self.history[player.idx()] = PICK_UP_SENTINEL;
    }
}

impl Serialize for OneAction {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = ser.serialize_seq(Some(self.history.len()))?;
        for i in 0..self.history.len() {
            ser.serialize_element(&self.last_action(PlayerIdx(i as u8)))?;
        }
        ser.end()
    }
}
