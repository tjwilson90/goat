use serde::{Serialize, Serializer};

use goat_api::{Card, PlayerIdx, RummyHistory};

pub struct OneAction {
    history: Box<[LastAction]>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum LastAction {
    Lead {
        lo: Card,
        hi: Card,
    },
    Play {
        lo: Card,
        hi: Card,
    },
    Kill {
        lo: Card,
        hi: Card,
    },
    #[serde(rename_all = "camelCase")]
    KillAndLead {
        kill_lo: Card,
        kill_hi: Card,
        lead_lo: Card,
        lead_hi: Card,
    },
    PickUp {
        lo: Card,
        hi: Card,
    },
    None,
}

impl OneAction {
    pub fn last_action(&self, player: PlayerIdx) -> LastAction {
        self.history[player.idx()]
    }
}

impl RummyHistory for OneAction {
    fn new(num_players: usize) -> Self {
        Self {
            history: vec![LastAction::None; num_players].into_boxed_slice(),
        }
    }

    fn lead(&mut self, player: PlayerIdx, lo: Card, hi: Card) {
        let action = &mut self.history[player.idx()];
        match *action {
            LastAction::Kill {
                lo: kill_lo,
                hi: kill_hi,
            } => {
                *action = LastAction::KillAndLead {
                    kill_lo,
                    kill_hi,
                    lead_lo: lo,
                    lead_hi: hi,
                }
            }
            _ => *action = LastAction::Lead { lo, hi },
        }
    }

    fn play(&mut self, player: PlayerIdx, lo: Card, hi: Card) {
        self.history[player.idx()] = LastAction::Play { lo, hi };
    }

    fn kill(&mut self, player: PlayerIdx, lo: Card, hi: Card) {
        self.history[player.idx()] = LastAction::Kill { lo, hi };
    }

    fn pick_up(&mut self, player: PlayerIdx, lo: Card, hi: Card) {
        self.history[player.idx()] = LastAction::PickUp { lo, hi };
    }
}

impl Serialize for OneAction {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.history.serialize(ser)
    }
}
