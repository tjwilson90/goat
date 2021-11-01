use std::mem;

use crate::{
    Card, Cards, Deck, GoatError, PlayerIdx, RummyPhase, Slot, WarHand, WarPlayKind, WarTrick,
};

#[derive(Clone, Debug)]
pub struct WarPhase<Deck, Hand, Trick> {
    pub deck: Deck,
    pub hands: Box<[Hand]>,
    pub won: Box<[Cards]>,
    pub trick: WarTrick,
    pub prev_trick: Trick,
}

impl<D: Deck, Hand: WarHand, Trick: Slot<WarTrick>> WarPhase<D, Hand, Trick> {
    pub fn play(&mut self, kind: WarPlayKind, card: Card) {
        self.trick.play(kind, card);
        if let Some(trick) = self.reset_trick_if_won() {
            self.prev_trick.set(trick);
        }
    }

    pub fn slough(&mut self, player: PlayerIdx, card: Card) -> Result<(), GoatError> {
        let hand = &mut self.hands[player.idx()];
        hand.check_has_card(card)?;
        if !self.trick.can_slough(player, card) {
            return Err(GoatError::IllegalSlough { card });
        }
        *hand -= card;
        self.trick.slough(player, card);
        Ok(())
    }

    pub fn is_finished(&self) -> bool {
        self.deck.is_empty()
            && self
                .trick
                .remaining_players()
                .any(|p| self.hands[p.idx()].is_empty())
    }

    pub fn switch_to_rummy(&self, trump: Card) -> RummyPhase<Hand::RummyHand> {
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
            .iter()
            .next()
            .map(|play| play.player())
            .unwrap_or(self.trick.next_player());
        RummyPhase::new(hands, next, trump)
    }

    fn reset_trick_if_won(&mut self) -> Option<WarTrick> {
        if let Some(winner) = self.trick.winner() {
            let won = &mut self.won[winner.idx()];
            let num_players = self.hands.len();
            let trick = mem::replace(&mut self.trick, WarTrick::new(winner, num_players));
            for card in trick.cards() {
                *won += card;
            }
            Some(trick)
        } else {
            None
        }
    }
}
