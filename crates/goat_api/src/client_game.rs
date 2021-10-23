use std::fmt::Debug;

use crate::{
    Cards, ClientDeck, ClientRummyHand, ClientWarHand, Event, GoatError, PlayerIdx, Rank,
    RummyPhase, UserId, WarHand, WarPhase, WarTrick,
};

#[derive(Debug)]
pub struct ClientGame {
    pub phase: ClientPhase,
    pub players: Vec<UserId>,
}

#[derive(Debug)]
pub enum ClientPhase {
    Unstarted,
    War(WarPhase<ClientDeck, ClientWarHand>),
    Rummy(RummyPhase<ClientRummyHand>),
    Complete(PlayerIdx),
}

impl ClientGame {
    pub fn new() -> Self {
        Self {
            phase: ClientPhase::Unstarted,
            players: Vec::new(),
        }
    }

    pub fn apply(&mut self, event: Event) -> Result<(), GoatError> {
        match event {
            Event::Join { user_id } => {
                self.players.push(user_id);
            }
            Event::Leave { player } => {
                self.players.swap_remove(player.0 as usize);
            }
            Event::Start { num_decks } => {
                let num_players = self.players.len();
                self.phase = ClientPhase::War(WarPhase {
                    deck: ClientDeck::new(num_decks),
                    hands: vec![ClientWarHand::new(); num_players].into_boxed_slice(),
                    won: vec![Cards::NONE; num_players].into_boxed_slice(),
                    trick: WarTrick::new(PlayerIdx(0), num_players),
                })
            }
            Event::PlayCard { card } => {
                let war = self.war()?;
                let player = war.trick.next_player();
                let hand = &mut war.hands[player.idx()];
                *hand -= card;
                war.play(card);
            }
            Event::PlayTop { card } => {
                let war = self.war()?;
                war.deck.draw();
                war.play(card);
            }
            Event::Slough { player, card } => {
                let war = self.war()?;
                war.slough(player, card)?;
            }
            Event::Draw { player, card } => {
                let war = self.war()?;
                war.deck.draw();
                war.hands[player.idx()] += card;
            }
            Event::RevealTrump { trump } => {
                let war = self.war()?;
                self.phase = ClientPhase::Rummy(war.switch_to_rummy(trump));
            }
            Event::OfferDreck { player, dreck } => {
                let rummy = self.rummy()?;
                let hand = &mut rummy.hands[player.idx()];
                *hand -= dreck;
            }
            Event::ReceiveDreck { player, dreck } => {
                let rummy = self.rummy()?;
                let hand = &mut rummy.hands[player.idx()];
                *hand += dreck;
            }
            Event::PlayRun { lo, hi } => {
                let rummy = self.rummy()?;
                rummy.advance_leader();
                rummy.play_run(rummy.next, lo, hi)?;
                if rummy.is_finished() {
                    self.phase = ClientPhase::Complete(rummy.next);
                }
            }
            Event::PickUp => {
                let rummy = self.rummy()?;
                rummy.advance_leader();
                rummy.pick_up(rummy.next)?;
            }
            Event::RedactedDraw { player } => {
                let war = self.war()?;
                war.deck.draw();
                war.hands[player.idx()] += 1;
            }
            Event::RedactedOfferDreck { player, dreck } => {
                let rummy = self.rummy()?;
                let hand = &mut rummy.hands[player.idx()];
                if dreck > 0 {
                    let cards = hand
                        .known
                        .remove_all(Cards::COMMON_DRECK + rummy.trump.with_rank(Rank::Six));
                    hand.unknown -= dreck - cards.len() as u8;
                }
            }
            Event::RedactedReceiveDreck { player, dreck } => {
                let rummy = self.rummy()?;
                let hand = &mut rummy.hands[player.idx()];
                hand.unknown += dreck;
            }
        }
        Ok(())
    }

    fn war(&mut self) -> Result<&mut WarPhase<ClientDeck, ClientWarHand>, GoatError> {
        match &mut self.phase {
            ClientPhase::War(war) => Ok(war),
            _ => Err(GoatError::InvalidAction),
        }
    }

    fn rummy(&mut self) -> Result<&mut RummyPhase<ClientRummyHand>, GoatError> {
        match &mut self.phase {
            ClientPhase::Rummy(rummy) => Ok(rummy),
            _ => Err(GoatError::InvalidAction),
        }
    }
}
