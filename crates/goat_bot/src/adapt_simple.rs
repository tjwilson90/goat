use goat_api::{
    Action, ClientDeck, ClientRummyHand, ClientWarHand, PlayerIdx, RummyPhase, WarPhase,
};

use crate::{strategy, Strategy};

pub struct AdaptSimple;

impl Strategy for AdaptSimple {
    fn war(&self, idx: PlayerIdx, war: &WarPhase<ClientDeck, ClientWarHand, ()>) -> Option<Action> {
        if war.hands.len() < 4 {
            strategy::war_duck(idx, war)
        } else {
            strategy::war_cover(idx, war)
        }
    }

    fn rummy(&self, idx: PlayerIdx, rummy: &RummyPhase<ClientRummyHand, ()>) -> Action {
        strategy::rummy_simple(idx, rummy)
    }
}
