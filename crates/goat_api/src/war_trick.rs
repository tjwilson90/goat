use std::cmp::Ordering;
use std::fmt::Debug;
use std::{fmt, mem};

use smallvec::SmallVec;

use crate::{
    Card, Cards, GoatError, PlayerIdx, Rank, ServerWarHand, WarHand, WarPlay, WarPlayKind,
};

static PLAYERS: &[PlayerIdx] = &[
    PlayerIdx(0),
    PlayerIdx(1),
    PlayerIdx(2),
    PlayerIdx(3),
    PlayerIdx(4),
    PlayerIdx(5),
    PlayerIdx(6),
    PlayerIdx(7),
    PlayerIdx(8),
    PlayerIdx(9),
    PlayerIdx(10),
    PlayerIdx(11),
    PlayerIdx(12),
    PlayerIdx(13),
    PlayerIdx(14),
    PlayerIdx(15),
];

#[derive(Clone)]
pub struct WarTrick {
    /// The index of the next player in players that needs to play a card.
    next: u8,

    /// The rank of the card currently winning the trick.
    rank: Rank,

    /// The ordered list of players who have/need to play in this round of the
    /// trick.
    players: SmallVec<[PlayerIdx; 16]>,

    /// The ordered list of players who have played the currently highest card
    /// in this round of the trick.
    winners: SmallVec<[PlayerIdx; 16]>,

    /// The cards that have been put into the trick from all rounds along with
    /// the player who played them and whether they were plays/sloughs/plays
    /// off the top.
    plays: SmallVec<[WarPlay; 12]>,

    /// A bit mask with set bits for each player who has not acknowledged the
    /// trick as complete.
    end_mask: u16,
}

impl WarTrick {
    pub fn new(leader: PlayerIdx, num_players: usize) -> Self {
        let mut players = SmallVec::new();
        players.extend_from_slice(&PLAYERS[leader.idx()..num_players]);
        players.extend_from_slice(&PLAYERS[..leader.idx()]);
        Self {
            next: 0,
            rank: Rank::Two,
            players,
            winners: SmallVec::new(),
            plays: SmallVec::new(),
            end_mask: if num_players == 16 {
                0xffff
            } else {
                (1 << num_players) - 1
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        self.plays.is_empty()
    }

    pub fn check_can_play(
        &self,
        player: PlayerIdx,
        hand: &ServerWarHand,
        card: Card,
    ) -> Result<(), GoatError> {
        if self.next_player() != Some(player) {
            return Err(GoatError::NotYourTurn { player });
        }
        hand.check_has_card(card)?;
        if let Some(rank) = self.rank() {
            if card.rank() != rank && hand.cards().any(|c| c.rank() == rank) {
                return Err(GoatError::MustMatchRank { rank });
            }
        }
        Ok(())
    }

    pub fn check_can_play_top(
        &self,
        player: PlayerIdx,
        hand: &ServerWarHand,
    ) -> Result<(), GoatError> {
        if self.next_player() != Some(player) {
            return Err(GoatError::NotYourTurn { player });
        }
        if let Some(rank) = self.rank() {
            if hand.cards().any(|c| c.rank() == rank) {
                return Err(GoatError::MustMatchRank { rank });
            }
        }
        Ok(())
    }

    pub fn check_can_slough(
        &self,
        player: PlayerIdx,
        hand: &ServerWarHand,
        card: Card,
    ) -> Result<(), GoatError> {
        hand.check_has_card(card)?;
        if self.ended(player) {
            return Err(GoatError::CannotSloughOnEndedTrick);
        }
        if self.rank() == Some(card.rank()) {
            if self.players[self.next as usize..].contains(&player) {
                return Err(GoatError::IllegalSlough { card });
            }
        } else {
            if !self
                .plays
                .iter()
                .any(|play| play.card.rank() == card.rank())
            {
                return Err(GoatError::IllegalSlough { card });
            }
        }
        Ok(())
    }

    pub fn next_player(&self) -> Option<PlayerIdx> {
        if self.players.len() == 1 {
            None
        } else {
            Some(self.players[self.next as usize])
        }
    }

    pub fn rank(&self) -> Option<Rank> {
        if self.next != 0 {
            Some(self.rank)
        } else {
            None
        }
    }

    pub fn remaining_players(&self) -> impl Iterator<Item = PlayerIdx> + '_ {
        self.players[self.next as usize..].iter().cloned()
    }

    pub fn winner(&self) -> Option<PlayerIdx> {
        if self.players.len() == 1 {
            Some(self.players[0])
        } else {
            None
        }
    }

    pub fn play(&mut self, kind: WarPlayKind, card: Card) {
        let player = self.players[self.next as usize];
        self.next += 1;
        match card.rank().cmp(&self.rank) {
            Ordering::Greater => {
                self.winners.clear();
                self.winners.push(player);
                self.rank = card.rank();
            }
            Ordering::Equal => self.winners.push(player),
            _ => {}
        }
        if self.players.len() == self.next as usize {
            self.players = mem::replace(&mut self.winners, SmallVec::new());
            self.next = 0;
            self.rank = Rank::Two;
        }
        self.plays.push(WarPlay::new(player, kind, card));
    }

    pub fn slough(&mut self, player: PlayerIdx, card: Card) {
        self.plays
            .push(WarPlay::new(player, WarPlayKind::Slough, card));
    }

    pub fn end_mask(&self) -> u16 {
        self.end_mask
    }

    pub fn ended(&self, player: PlayerIdx) -> bool {
        self.end_mask & (1 << player.0) == 0
    }

    pub fn end(&mut self, player: PlayerIdx) -> bool {
        self.end_mask &= !(1 << player.0);
        self.end_mask == 0
    }

    pub fn cards(&self) -> impl Iterator<Item = Card> + '_ {
        self.plays.iter().map(|p| p.card)
    }

    pub fn plays(&self) -> &[WarPlay] {
        &*self.plays
    }
}

impl Debug for WarTrick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_struct("");
        f.field("next", &self.next_player());
        f.field("players", &self.players);
        if let Some(rank) = self.rank() {
            f.field("rank", &rank);
            f.field("winners", &self.winners);
        }
        if !self.plays.is_empty() {
            f.field("cards", &self.cards().collect::<Cards>());
        }
        f.field("end_mask", &format!("{:b}", self.end_mask));
        f.finish()
    }
}
