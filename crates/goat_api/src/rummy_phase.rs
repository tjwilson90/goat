use rand::prelude::{SliceRandom, StdRng};
use rand::SeedableRng;
use smallvec::SmallVec;

use crate::{Card, Cards, Event, GoatError, PlayerIdx, Rank, RummyHand, RummyHistory, RummyTrick};

#[derive(Clone, Debug)]
pub struct RummyPhase<Hand, History> {
    pub hands: Box<[Hand]>,
    pub trick: RummyTrick,
    pub next: PlayerIdx,
    pub trump: Card,
    pub pick_ups: u64,
    pub history: History,
}

impl<Hand: RummyHand, History: RummyHistory> RummyPhase<Hand, History> {
    pub fn new(hands: Box<[Hand]>, next: PlayerIdx, trump: Card) -> Self {
        let num_players = hands.len();
        let mut phase = Self {
            hands,
            trick: RummyTrick::new(0),
            next,
            trump,
            pick_ups: 0,
            history: History::new(num_players),
        };
        phase.reset_trick();
        phase
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
        self.history.play(player, lo, hi);
        let killed = self.trick.play(lo, hi);
        if killed {
            self.reset_trick();
            self.pick_ups = 0;
        } else {
            self.next = PlayerIdx(self.next.0 + 1);
        }
        self.advance_leader();
        Ok(())
    }

    pub fn pick_up(&mut self, player: PlayerIdx) -> Result<bool, GoatError> {
        if self.next != player {
            return Err(GoatError::NotYourTurn { player });
        }
        if self.trick.is_empty() {
            return Err(GoatError::CannotPickUpFromEmptyTrick);
        }
        let hand = &mut self.hands[player.idx()];
        let (lo, hi) = self.trick.pick_up();
        *hand += Cards::range(lo, hi);
        self.history.pick_up(player);
        if self.trick.is_empty() {
            self.reset_trick();
        }
        let complete = self.increment_pick_ups(player);
        self.next = PlayerIdx(self.next.0 + 1);
        self.advance_leader();
        Ok(complete)
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

    pub fn finished_receiving_dreck(&self) -> bool {
        self.hands.iter().map(|hand| hand.len()).sum::<usize>() % 52 == 51
    }

    fn increment_pick_ups(&mut self, player: PlayerIdx) -> bool {
        let pick_ups = (self.pick_ups >> (4 * player.0)) & 0xf;
        if pick_ups < 10 {
            self.pick_ups += 1 << (4 * player.0);
        }
        if pick_ups == 9 {
            for i in 0..self.hands.len() {
                if !self.hands[i].is_empty() && (self.pick_ups >> (4 * i)) & 0xf != 10 {
                    return false;
                }
            }
            return true;
        }
        false
    }

    pub fn reset_trick(&mut self) {
        self.trick = RummyTrick::new(self.hands.iter().filter(|h| !h.is_empty()).count());
    }
}

impl RummyPhase<Cards, ()> {
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
        } else {
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
        }
        self.reset_trick();
        self.advance_leader();
    }
}
