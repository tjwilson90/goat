use std::fmt::Debug;

use crate::{
    Cards, ClientDeck, ClientRummyHand, ClientWarHand, Event, GoatError, GoatPhase, PlayerIdx,
    PreviousTrick, Rank, RummyHistory, RummyPhase, UserId, WarHand, WarPhase, WarTrick,
};

#[derive(Clone, Debug)]
pub struct ClientGame<PrevTrick, History> {
    pub phase: ClientPhase<PrevTrick, History>,
    pub players: Vec<UserId>,
}

#[derive(Clone, Debug)]
pub enum ClientPhase<PrevTrick, History> {
    Unstarted,
    War(WarPhase<ClientDeck, ClientWarHand, PrevTrick>),
    Rummy(RummyPhase<ClientRummyHand, History>),
    Goat(GoatPhase),
}

impl<PrevTrick: PreviousTrick, History: RummyHistory> ClientGame<PrevTrick, History> {
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
                self.players.swap_remove(player.idx());
            }
            Event::Start { num_decks } => {
                let num_players = self.players.len();
                self.phase = ClientPhase::War(WarPhase {
                    deck: ClientDeck::new(num_decks),
                    hands: vec![ClientWarHand::new(); num_players].into_boxed_slice(),
                    won: vec![Cards::NONE; num_players].into_boxed_slice(),
                    trick: WarTrick::new(PlayerIdx(0), num_players),
                    prev_trick: PreviousTrick::empty(),
                })
            }
            Event::PlayCard { card } => {
                let war = self.war()?;
                let player = war.trick.next_player().unwrap();
                war.play_from_hand(player, card);
            }
            Event::PlayTop { card } => {
                let war = self.war()?;
                war.deck.draw();
                war.play_from_top(card);
            }
            Event::Slough { player, card } => {
                let war = self.war()?;
                war.slough(player, card);
            }
            Event::Draw { player, card } => {
                let war = self.war()?;
                war.deck.draw();
                war.hands[player.idx()] += card;
            }
            Event::FinishSloughing { player } => {
                let war = self.war()?;
                war.end_trick(player)?;
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
                if rummy.finished_receiving_dreck() {
                    rummy.reset_trick();
                    rummy.advance_leader();
                }
            }
            Event::PlayRun { lo, hi } => {
                let rummy = self.rummy()?;
                rummy.play_run(rummy.next, lo, hi)?;
                if rummy.is_finished() {
                    self.phase = ClientPhase::Goat(GoatPhase::new(rummy.next));
                }
            }
            Event::PickUp => {
                let rummy = self.rummy()?;
                let player = rummy.next;
                if rummy.pick_up(player)? {
                    self.phase = ClientPhase::Goat(GoatPhase::new(player));
                }
            }
            Event::Goat { noise } => {
                let goat = self.goat()?;
                goat.noise = Some(noise);
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
                if rummy.finished_receiving_dreck() {
                    rummy.reset_trick();
                    rummy.advance_leader();
                }
            }
        }
        Ok(())
    }

    fn war(&mut self) -> Result<&mut WarPhase<ClientDeck, ClientWarHand, PrevTrick>, GoatError> {
        match &mut self.phase {
            ClientPhase::War(war) => Ok(war),
            _ => Err(GoatError::InvalidAction),
        }
    }

    fn rummy(&mut self) -> Result<&mut RummyPhase<ClientRummyHand, History>, GoatError> {
        match &mut self.phase {
            ClientPhase::Rummy(rummy) => Ok(rummy),
            _ => Err(GoatError::InvalidAction),
        }
    }

    fn goat(&mut self) -> Result<&mut GoatPhase, GoatError> {
        match &mut self.phase {
            ClientPhase::Goat(goat) => Ok(goat),
            _ => Err(GoatError::InvalidAction),
        }
    }
}
