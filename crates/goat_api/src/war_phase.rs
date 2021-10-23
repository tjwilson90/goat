use std::mem;

use crate::{Card, Cards, Deck, GoatError, PlayerIdx, RummyPhase, WarHand, WarTrick};

#[derive(Debug)]
pub struct WarPhase<D, Hand> {
    pub deck: D,
    pub hands: Box<[Hand]>,
    pub won: Box<[Cards]>,
    pub trick: WarTrick,
}

impl<D: Deck, Hand: WarHand> WarPhase<D, Hand> {
    pub fn play(&mut self, card: Card) {
        self.trick.play(card);
        self.reset_trick_if_won();
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
        for (p, c) in self.trick.players_and_plays() {
            hands[p.idx()] += c;
        }
        let next = self
            .trick
            .players_and_plays()
            .next()
            .map(|(p, _)| p)
            .unwrap_or(self.trick.next_player());
        RummyPhase::new(hands, next, trump)
    }

    fn reset_trick_if_won(&mut self) {
        if let Some(winner) = self.trick.winner() {
            let won = &mut self.won[winner.idx()];
            let num_players = self.hands.len();
            let trick = mem::replace(&mut self.trick, WarTrick::new(winner, num_players));
            for card in trick.plays() {
                *won += card;
            }
        }
    }
}
