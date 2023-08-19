use async_trait::async_trait;
use goat_api::{
    Action, Cards, ClientDeck, ClientRummyHand, ClientWarHand, PlayerIdx, RummyPhase, WarPhase,
};

use crate::{strategy, Strategy};

pub struct DuckSimple;

#[async_trait]
impl Strategy for DuckSimple {
    fn war(&self, idx: PlayerIdx, war: &WarPhase<ClientDeck, ClientWarHand, ()>) -> Option<Action> {
        strategy::war_duck(idx, war)
    }

    async fn rummy(&self, rummy: &RummyPhase<ClientRummyHand, Cards>) -> Action {
        strategy::rummy_simple(rummy)
    }
}
