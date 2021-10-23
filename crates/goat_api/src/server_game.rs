use rand::prelude::{SeedableRng, SliceRandom, StdRng};

use crate::{
    Action, Card, Cards, Event, GoatError, PlayerIdx, RummyPhase, ServerWarHand, UserId, WarHand,
    WarPhase, WarTrick,
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
    War(WarPhase<Vec<Card>, ServerWarHand>),
    Rummy(RummyPhase<Cards>),
    Complete(PlayerIdx),
}

macro_rules! switch_if_finished {
    ($self:ident, $war:ident, $events:ident, $seed:ident) => {
        if $war.is_finished() {
            let trump = $war.deck[0];
            $events.push(Event::RevealTrump { trump });
            let mut rummy = $war.switch_to_rummy(trump);
            rummy.distribute_dreck($events, $seed);
            $self.phase = ServerPhase::Rummy(rummy);
        }
    };
}

impl ServerGame {
    pub fn new(seed: u64) -> Self {
        Self {
            phase: ServerPhase::Unstarted,
            players: Vec::new(),
            events: Vec::new(),
            seed,
        }
    }

    pub fn player(&self, user_id: UserId) -> Result<PlayerIdx, GoatError> {
        match self.players.iter().position(|p| *p == user_id) {
            Some(idx) => Ok(PlayerIdx(idx as u8)),
            None => return Err(GoatError::InvalidPlayer { user_id }),
        }
    }

    pub fn started(&self) -> bool {
        match self.phase {
            ServerPhase::Unstarted => false,
            _ => true,
        }
    }

    pub fn complete(&self) -> Option<PlayerIdx> {
        match self.phase {
            ServerPhase::Complete(player) => Some(player),
            _ => None,
        }
    }

    pub fn apply(&mut self, user_id: UserId, action: Action) -> Result<(), GoatError> {
        match action {
            Action::Join { user_id } => {
                match self.phase {
                    ServerPhase::Unstarted => {}
                    _ => return Err(GoatError::InvalidAction),
                }
                if let Err(_) = self.player(user_id) {
                    if self.players.len() == 16 {
                        return Err(GoatError::TooManyPlayers);
                    }
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
                if num_decks < 1 || num_decks > 3 {
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
                });
                self.events.push(Event::Start { num_decks });
            }
            Action::PlayCard { card } => {
                let player = self.player(user_id)?;
                let (war, events, seed) = self.war()?;
                if war.trick.next_player() != player {
                    return Err(GoatError::NotYourTurn { player });
                }
                let hand = &mut war.hands[player.idx()];
                hand.check_has_card(card)?;
                if let Some(rank) = war.trick.rank() {
                    if card.rank() != rank && hand.cards().any(|c| c.rank() == rank) {
                        return Err(GoatError::MustMatchRank { rank });
                    }
                }
                *hand -= card;
                war.play(card);
                events.push(Event::PlayCard { card });
                switch_if_finished!(self, war, events, seed);
            }
            Action::PlayTop => {
                let player = self.player(user_id)?;
                let (war, events, seed) = self.war()?;
                if war.trick.next_player() != player {
                    return Err(GoatError::NotYourTurn { player });
                }
                let hand = &war.hands[player.idx()];
                if let Some(rank) = war.trick.rank() {
                    if hand.cards().any(|c| c.rank() == rank) {
                        return Err(GoatError::MustMatchRank { rank });
                    }
                }
                if war.deck.len() <= 1 {
                    return Err(GoatError::CannotPlayFromEmptyDeck);
                }
                let card = war.deck.pop().unwrap();
                war.play(card);
                events.push(Event::PlayTop { card });
                switch_if_finished!(self, war, events, seed);
            }
            Action::Slough { card } => {
                let player = self.player(user_id)?;
                let (war, events, seed) = self.war()?;
                war.slough(player, card)?;
                events.push(Event::Slough { player, card });
                switch_if_finished!(self, war, events, seed);
            }
            Action::Draw => {
                let player = self.player(user_id)?;
                let (war, events, seed) = self.war()?;
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
                switch_if_finished!(self, war, events, seed);
            }
            Action::PlayRun { lo, hi } => {
                let player = self.player(user_id)?;
                let (rummy, events) = self.rummy()?;
                rummy.play_run(player, lo, hi)?;
                events.push(Event::PlayRun { lo, hi });
                if rummy.is_finished() {
                    self.phase = ServerPhase::Complete(rummy.next);
                }
            }
            Action::PickUp => {
                let player = self.player(user_id)?;
                let (rummy, events) = self.rummy()?;
                rummy.pick_up(player)?;
                events.push(Event::PickUp);
            }
        };
        Ok(())
    }

    fn war(
        &mut self,
    ) -> Result<
        (
            &mut WarPhase<Vec<Card>, ServerWarHand>,
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

    fn rummy(&mut self) -> Result<(&mut RummyPhase<Cards>, &mut Vec<Event>), GoatError> {
        match &mut self.phase {
            ServerPhase::Rummy(rummy) => Ok((rummy, &mut self.events)),
            _ => Err(GoatError::InvalidAction),
        }
    }
}
