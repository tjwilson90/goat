use goat_api::{
    Action, ClientDeck, ClientRummyHand, ClientWarHand, PlayerIdx, RummyPhase, WarPhase,
};

use crate::{strategy, Strategy};

pub struct CoverSimple;

impl Strategy for CoverSimple {
    fn war(&self, idx: PlayerIdx, war: &WarPhase<ClientDeck, ClientWarHand, ()>) -> Option<Action> {
        strategy::war_cover(idx, war)
    }

    fn rummy(&self, idx: PlayerIdx, rummy: &RummyPhase<ClientRummyHand, ()>) -> Action {
        strategy::rummy_simple(idx, rummy)
    }
}
