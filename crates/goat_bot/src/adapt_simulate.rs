use goat_api::{
    Action, Cards, ClientDeck, ClientRummyHand, ClientWarHand, PlayerIdx, RummyPhase, WarPhase,
};

use crate::{strategy, Strategy};

pub struct AdaptSimulate;

impl Strategy for AdaptSimulate {
    fn war(&self, idx: PlayerIdx, war: &WarPhase<ClientDeck, ClientWarHand, ()>) -> Option<Action> {
        if war.hands.len() < 4 {
            strategy::war_duck(idx, war)
        } else {
            strategy::war_cover(idx, war)
        }
    }

    fn rummy(&self, rummy: &RummyPhase<ClientRummyHand, Cards>) -> Action {
        strategy::rummy_simulate(rummy)
    }
}
