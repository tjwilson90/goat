use rand::prelude::{SeedableRng, SliceRandom, StdRng};

use crate::{
    Action, Card, Cards, Event, GoatError, GoatPhase, PlayerIdx, RummyPhase, ServerWarHand, UserId,
    WarHand, WarPhase, WarTrick,
};

#[derive(Debug)]
pub struct ServerGame {
    pub phase: ServerPhase,
    pub players: Vec<UserId>,
    pub events: Vec<Event>,
    pub seed: u64,
}

#[derive(Debug)]
pub enum ServerPhase {
    Unstarted,
    War(WarPhase<Vec<Card>, ServerWarHand, ()>),
    Rummy(RummyPhase<Cards, ()>),
    Goat(GoatPhase),
}

impl ServerGame {
    pub fn new(seed: u64) -> Self {
        Self {
            phase: ServerPhase::Unstarted,
            players: Vec::with_capacity(4),
            events: Vec::with_capacity(128),
            seed,
        }
    }

    pub fn player(&self, user_id: UserId) -> Result<PlayerIdx, GoatError> {
        match self.players.iter().position(|p| *p == user_id) {
            Some(idx) => Ok(PlayerIdx(idx as u8)),
            None => Err(GoatError::InvalidPlayer { user_id }),
        }
    }

    pub fn active(&self) -> bool {
        matches!(self.phase, ServerPhase::War(_) | ServerPhase::Rummy(_))
    }

    pub fn apply(&mut self, user_id: UserId, action: Action) -> Result<(), GoatError> {
        match action {
            Action::Join { user_id } => {
                match self.phase {
                    ServerPhase::Unstarted => {}
                    _ => return Err(GoatError::InvalidAction),
                }
                if self.player(user_id).is_err() {
                    self.players.push(user_id);
                    self.events.push(Event::Join { user_id });
                }
            }
            Action::Leave { player } => {
                match self.phase {
                    ServerPhase::Unstarted => {}
                    _ => return Err(GoatError::InvalidAction),
                }
                self.players.swap_remove(player.idx());
                self.events.push(Event::Leave { player });
            }
            Action::Start { num_decks } => {
                match self.phase {
                    ServerPhase::Unstarted => {}
                    _ => return Err(GoatError::InvalidAction),
                }
                if !(3..=15).contains(&self.players.len()) {
                    return Err(GoatError::InvalidNumberOfPlayers);
                }
                if !(1..=3).contains(&num_decks) {
                    return Err(GoatError::InvalidNumberOfDecks);
                }
                let num_players = self.players.len();
                let mut deck: Vec<_> = (Cards::ONE_DECK * num_decks as usize).into_iter().collect();
                deck.shuffle(&mut StdRng::seed_from_u64(self.seed));
                self.phase = ServerPhase::War(WarPhase {
                    deck,
                    hands: vec![ServerWarHand::new(); num_players].into_boxed_slice(),
                    won: vec![Cards::NONE; num_players].into_boxed_slice(),
                    trick: WarTrick::new(PlayerIdx(0), num_players),
                    prev_trick: (),
                });
                self.events.push(Event::Start { num_decks });
            }
            Action::PlayCard { card } => {
                let player = self.player(user_id)?;
                let (war, events, _) = self.war()?;
                if war.is_finished() {
                    return Err(GoatError::CannotPlayOnFinishedTrick);
                }
                let hand = &war.hands[player.idx()];
                war.trick.check_can_play(player, hand, card)?;
                war.play_from_hand(player, card);
                events.push(Event::PlayCard { card });
            }
            Action::PlayTop => {
                let player = self.player(user_id)?;
                let (war, events, _) = self.war()?;
                if war.deck.len() <= 1 {
                    return Err(GoatError::CannotPlayFromEmptyDeck);
                }
                let hand = &war.hands[player.idx()];
                war.trick.check_can_play_top(player, hand)?;
                let card = war.deck.pop().unwrap();
                war.play_from_top(card);
                events.push(Event::PlayTop { card });
            }
            Action::Slough { card } => {
                let player = self.player(user_id)?;
                let (war, events, _) = self.war()?;
                let hand = &war.hands[player.idx()];
                war.trick.check_can_slough(player, hand, card)?;
                war.slough(player, card);
                events.push(Event::Slough { player, card });
            }
            Action::Draw => {
                let player = self.player(user_id)?;
                let (war, events, _) = self.war()?;
                let hand = &mut war.hands[player.idx()];
                if hand.len() == 3 {
                    return Err(GoatError::CannotDrawMoreThanThreeCards);
                }
                if war.deck.len() <= 1 {
                    return Err(GoatError::CannotDrawFromEmptyDeck);
                }
                let card = war.deck.pop().unwrap();
                *hand += card;
                events.push(Event::Draw { player, card });
            }
            Action::FinishTrick => {
                let player = self.player(user_id)?;
                let (war, events, seed) = self.war()?;
                let complete = war.finish_trick(player)?;
                events.push(Event::FinishTrick { player });
                if complete && war.is_finished() {
                    let trump = war.deck[0];
                    events.push(Event::RevealTrump { trump });
                    let mut rummy = war.switch_to_rummy(trump);
                    rummy.distribute_dreck(events, seed);
                    self.phase = ServerPhase::Rummy(rummy);
                }
            }
            Action::PlayRun { lo, hi } => {
                let player = self.player(user_id)?;
                let (rummy, events) = self.rummy()?;
                rummy.play_run(player, lo, hi)?;
                events.push(Event::PlayRun { lo, hi });
                if rummy.is_finished() {
                    self.phase = ServerPhase::Goat(GoatPhase::new(rummy.next));
                }
            }
            Action::PickUp => {
                let player = self.player(user_id)?;
                let (rummy, events) = self.rummy()?;
                let complete = rummy.pick_up(player)?;
                events.push(Event::PickUp);
                if complete {
                    self.phase = ServerPhase::Goat(GoatPhase::new(player));
                }
            }
            Action::Goat { noise } => {
                let player = self.player(user_id)?;
                let (goat, events) = self.goat()?;
                if player != goat.goat {
                    return Err(GoatError::NoFreeShows);
                }
                goat.noise = Some(noise);
                events.push(Event::Goat { noise });
            }
        };
        Ok(())
    }

    fn war(
        &mut self,
    ) -> Result<
        (
            &mut WarPhase<Vec<Card>, ServerWarHand, ()>,
            &mut Vec<Event>,
            u64,
        ),
        GoatError,
    > {
        match &mut self.phase {
            ServerPhase::War(war) => Ok((war, &mut self.events, self.seed)),
            _ => Err(GoatError::InvalidAction),
        }
    }

    fn rummy(&mut self) -> Result<(&mut RummyPhase<Cards, ()>, &mut Vec<Event>), GoatError> {
        match &mut self.phase {
            ServerPhase::Rummy(rummy) => Ok((rummy, &mut self.events)),
            _ => Err(GoatError::InvalidAction),
        }
    }

    fn goat(&mut self) -> Result<(&mut GoatPhase, &mut Vec<Event>), GoatError> {
        match &mut self.phase {
            ServerPhase::Goat(goat) => Ok((goat, &mut self.events)),
            _ => Err(GoatError::InvalidAction),
        }
    }
}
