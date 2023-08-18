use std::fmt::Debug;
use std::mem;

use crate::{
    Card, Cards, Deck, GoatError, PlayerIdx, PreviousTrick, RummyHistory, RummyPhase, WarHand,
    WarPlayKind, WarTrick,
};

#[derive(Clone, Debug)]
pub struct WarPhase<Deck, Hand, Trick> {
    pub deck: Deck,
    pub hands: Box<[Hand]>,
    pub won: Box<[Cards]>,
    pub trick: WarTrick,
    pub prev_trick: Trick,
}

impl<D: Deck, Hand: WarHand, Trick: PreviousTrick> WarPhase<D, Hand, Trick> {
    pub fn play_from_hand(&mut self, player: PlayerIdx, card: Card) {
        self.trick.play(WarPlayKind::PlayHand, card);
        let hand = &mut self.hands[player.idx()];
        *hand -= card;
    }

    pub fn play_from_top(&mut self, card: Card) {
        self.trick.play(WarPlayKind::PlayTop, card);
    }

    pub fn slough(&mut self, player: PlayerIdx, card: Card) {
        let hand = &mut self.hands[player.idx()];
        self.trick.slough(player, card);
        *hand -= card;
    }

    pub fn finish_trick(&mut self, player: PlayerIdx) -> Result<bool, GoatError> {
        let winner = self.trick.winner();
        if winner.is_none() && !self.is_finished() {
            return Err(GoatError::CannotFinishIncompleteTrick);
        }
        let complete_trick = self.trick.end(player);
        if complete_trick {
            if let Some(winner) = winner {
                let won = &mut self.won[winner.idx()];
                let num_players = self.hands.len();
                let trick = mem::replace(&mut self.trick, WarTrick::new(winner, num_players));
                for card in trick.cards() {
                    *won += card;
                }
                self.prev_trick.set(trick);
            }
        }
        Ok(complete_trick)
    }

    pub fn is_finished(&self) -> bool {
        self.deck.cards_remaining() == 0
            && self
                .trick
                .remaining_players()
                .any(|p| self.hands[p.idx()].is_empty())
    }

    pub fn switch_to_rummy<History: RummyHistory>(
        &self,
        trump: Card,
    ) -> RummyPhase<Hand::RummyHand, History> {
        let mut hands = self
            .won
            .iter()
            .zip(self.hands.iter())
            .map(|(won, hand)| hand.merge_into_rummy_hand(*won))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        for play in self.trick.plays() {
            hands[play.player().idx()] += play.card;
        }
        let next = self
            .trick
            .plays()
            .get(0)
            .map(|play| play.player())
            .unwrap_or_else(|| self.trick.next_player().unwrap());
        RummyPhase::new(hands, next, trump)
    }
}
