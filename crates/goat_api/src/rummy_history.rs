use crate::{Card, Cards, PlayerIdx};

pub trait RummyHistory {
    fn new(num_players: usize) -> Self;
    fn lead(&mut self, player: PlayerIdx, lo: Card, hi: Card);
    fn play(&mut self, player: PlayerIdx, lo: Card, hi: Card);
    fn kill(&mut self, player: PlayerIdx, lo: Card, hi: Card);
    fn pick_up(&mut self, player: PlayerIdx, lo: Card, hi: Card);
}

impl RummyHistory for () {
    fn new(_: usize) -> Self {}

    fn lead(&mut self, _: PlayerIdx, _: Card, _: Card) {}

    fn play(&mut self, _: PlayerIdx, _: Card, _: Card) {}

    fn kill(&mut self, _: PlayerIdx, _: Card, _: Card) {}

    fn pick_up(&mut self, _: PlayerIdx, _: Card, _: Card) {}
}

impl RummyHistory for Cards {
    fn new(_: usize) -> Self {
        Cards::NONE
    }

    fn lead(&mut self, _: PlayerIdx, lo: Card, hi: Card) {
        *self += Cards::range(lo, hi);
    }

    fn play(&mut self, _: PlayerIdx, lo: Card, hi: Card) {
        *self += Cards::range(lo, hi);
    }

    fn kill(&mut self, _: PlayerIdx, lo: Card, hi: Card) {
        *self += Cards::range(lo, hi);
    }

    fn pick_up(&mut self, _: PlayerIdx, lo: Card, hi: Card) {
        *self -= Cards::range(lo, hi);
    }
}
