use goat_api::{
    Action, ClientDeck, ClientRummyHand, ClientWarHand, PlayerIdx, RummyPhase, WarPhase,
};

use crate::{strategy, Strategy};

pub struct DuckSimple;

impl Strategy for DuckSimple {
    fn war(&self, idx: PlayerIdx, war: &WarPhase<ClientDeck, ClientWarHand, ()>) -> Option<Action> {
        strategy::war_duck(idx, war)
    }

    fn rummy(&self, idx: PlayerIdx, rummy: &RummyPhase<ClientRummyHand, ()>) -> Action {
        strategy::rummy_simple(idx, rummy)
    }
}
