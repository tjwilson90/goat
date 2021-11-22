use goat_api::{
    Action, ClientDeck, ClientRummyHand, ClientWarHand, PlayerIdx, RummyPhase, WarPhase,
};

use crate::{strategy, Strategy};

pub struct PlayTopSimple;

impl Strategy for PlayTopSimple {
    fn war(&self, idx: PlayerIdx, war: &WarPhase<ClientDeck, ClientWarHand, ()>) -> Option<Action> {
        strategy::war_play_top(idx, war)
    }

    fn rummy(&self, idx: PlayerIdx, rummy: &RummyPhase<ClientRummyHand, ()>) -> Action {
        strategy::rummy_simple(idx, rummy)
    }
}
