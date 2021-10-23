use rand::prelude::{SliceRandom, StdRng};
use rand::SeedableRng;
use smallvec::SmallVec;

use crate::{Card, Cards, Event, GoatError, PlayerIdx, Rank, RummyHand, RummyTrick};

#[derive(Debug)]
pub struct RummyPhase<Hand> {
    pub hands: Box<[Hand]>,
    pub trick: RummyTrick,
    pub next: PlayerIdx,
    pub trump: Card,
}

impl<Hand: RummyHand> RummyPhase<Hand> {
    pub fn new(hands: Box<[Hand]>, next: PlayerIdx, trump: Card) -> Self {
        let trick = Self::new_trick(&*hands);
        Self {
            hands,
            trick,
            next,
            trump,
        }
    }

    pub fn play_run(&mut self, player: PlayerIdx, lo: Card, hi: Card) -> Result<(), GoatError> {
        if self.next != player {
            return Err(GoatError::NotYourTurn { player });
        }
        if lo.suit() != hi.suit() || lo.rank() > hi.rank() {
            return Err(GoatError::InvalidRange { lo, hi });
        }
        let hand = &mut self.hands[player.idx()];
        hand.check_can_play(lo, hi)?;
        if !self.trick.can_play(lo, self.trump.suit()) {
            return Err(GoatError::CannotPlayRange { lo });
        }
        *hand -= Cards::range(lo, hi);
        let killed = self.trick.play(lo, hi);
        if killed {
            self.trick = Self::new_trick(&*self.hands);
        } else {
            self.next = PlayerIdx(self.next.0 + 1);
        }
        self.advance_leader();
        Ok(())
    }

    pub fn pick_up(&mut self, player: PlayerIdx) -> Result<(), GoatError> {
        if self.next != player {
            return Err(GoatError::NotYourTurn { player });
        }
        let hand = &mut self.hands[player.idx()];
        let (lo, hi) = self.trick.pick_up();
        *hand += Cards::range(lo, hi);
        if self.trick.is_empty() {
            self.trick = Self::new_trick(&*self.hands);
        }
        self.next = PlayerIdx(self.next.0 + 1);
        self.advance_leader();
        Ok(())
    }

    pub fn is_finished(&self) -> bool {
        self.hands.iter().filter(|hand| !hand.is_empty()).count() == 1
    }

    pub fn advance_leader(&mut self) {
        let mut next = self.next.idx();
        loop {
            if next == self.hands.len() {
                next = 0;
            }
            if !self.hands[next].is_empty() {
                break;
            }
            next += 1;
        }
        self.next = PlayerIdx(next as u8);
    }

    fn new_trick(hands: &[Hand]) -> RummyTrick {
        RummyTrick::new(hands.iter().filter(|h| !h.is_empty()).count())
    }
}

impl RummyPhase<Cards> {
    pub fn distribute_dreck(&mut self, events: &mut Vec<Event>, seed: u64) {
        let mut dreck_players: SmallVec<[PlayerIdx; 16]> = self
            .hands
            .iter()
            .enumerate()
            .filter_map(|(player, cards)| {
                if cards.len() < 6 {
                    Some(PlayerIdx(player as u8))
                } else {
                    None
                }
            })
            .collect();
        if dreck_players.is_empty() {
            return;
        }
        let dreck_cards = Cards::COMMON_DRECK + self.trump.with_rank(Rank::Six);
        let mut all_dreck = Cards::NONE;
        self.hands
            .iter_mut()
            .enumerate()
            .for_each(|(player, hand)| {
                let dreck = hand.remove_all(dreck_cards);
                if !dreck.is_empty() {
                    all_dreck += dreck;
                    events.push(Event::OfferDreck {
                        player: PlayerIdx(player as u8),
                        dreck,
                    });
                }
            });
        if dreck_players.len() == 1 {
            let player = dreck_players[0];
            self.hands[player.idx()] += all_dreck;
            events.push(Event::ReceiveDreck {
                player,
                dreck: all_dreck,
            });
            return;
        }
        let mut all_dreck: Vec<_> = all_dreck.into_iter().collect();
        let mut rng = StdRng::seed_from_u64(seed ^ 1);
        dreck_players.shuffle(&mut rng);
        all_dreck.shuffle(&mut rng);
        let mut dreck_players = dreck_players.into_iter();
        let mut all_dreck = all_dreck.into_iter();
        while let Some(player) = dreck_players.next() {
            let len = all_dreck.len() / (1 + dreck_players.len());
            let dreck = all_dreck.by_ref().take(len).collect();
            self.hands[player.idx()] += dreck;
            events.push(Event::ReceiveDreck { player, dreck });
        }
        self.advance_leader();
    }
}
