use std::cmp::Ordering;
use std::fmt::Debug;
use std::{fmt, mem};

use smallvec::SmallVec;

use crate::{Card, Cards, PlayerIdx, Rank};

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

pub struct WarTrick {
    next: u8,
    rank: Rank,
    players: SmallVec<[PlayerIdx; 16]>,
    winners: SmallVec<[PlayerIdx; 16]>,
    plays: SmallVec<[(PlayerIdx, Card); 12]>,
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
        }
    }

    pub fn can_slough(&self, player: PlayerIdx, card: Card) -> bool {
        if self.rank() == Some(card.rank()) {
            !self.players[self.next as usize..].contains(&player)
        } else {
            self.plays.iter().any(|(_, c)| c.rank() == card.rank())
        }
    }

    pub fn next_player(&self) -> PlayerIdx {
        self.players[self.next as usize]
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

    pub fn play(&mut self, card: Card) {
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
        self.plays.push((player, card));
    }

    pub fn slough(&mut self, player: PlayerIdx, card: Card) {
        self.plays.push((player, card));
    }

    pub fn plays(&self) -> impl Iterator<Item = Card> + '_ {
        self.plays.iter().map(|(_, c)| c).cloned()
    }

    pub fn players_and_plays(&self) -> impl Iterator<Item = (PlayerIdx, Card)> + '_ {
        self.plays.iter().cloned()
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
            f.field("cards", &self.plays().collect::<Cards>());
        }
        f.finish()
    }
}
