use goat_api::{
    Action, Cards, ClientDeck, ClientRummyHand, ClientWarHand, PlayerIdx, RummyPhase, WarPhase,
};

use crate::{strategy, Strategy};

pub struct PlayTopSimple;

impl Strategy for PlayTopSimple {
    fn war(&self, idx: PlayerIdx, war: &WarPhase<ClientDeck, ClientWarHand, ()>) -> Option<Action> {
        strategy::war_play_top(idx, war)
    }

    fn rummy(&self, rummy: &RummyPhase<ClientRummyHand, Cards>) -> Action {
        strategy::rummy_simple(rummy)
    }
}
