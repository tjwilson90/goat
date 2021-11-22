use crate::{Card, PlayerIdx};

pub trait RummyHistory {
    fn new(num_players: usize) -> Self;
    fn play(&mut self, player: PlayerIdx, lo: Card, hi: Card);
    fn pick_up(&mut self, player: PlayerIdx);
}

impl RummyHistory for () {
    fn new(_: usize) -> Self {
        ()
    }

    fn play(&mut self, _: PlayerIdx, _: Card, _: Card) {}

    fn pick_up(&mut self, _: PlayerIdx) {}
}
