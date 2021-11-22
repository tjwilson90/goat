use std::mem;

use crate::{Card, PlayerIdx};

#[derive(Copy, Clone)]
pub struct WarPlay {
    player_and_kind: u8,
    pub card: Card,
}

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum WarPlayKind {
    PlayHand = 0,
    PlayTop,
    Slough,
}

impl WarPlay {
    pub fn new(player: PlayerIdx, kind: WarPlayKind, card: Card) -> Self {
        Self {
            player_and_kind: player.0 | ((kind as u8) << 4),
            card,
        }
    }

    pub fn player(self) -> PlayerIdx {
        PlayerIdx(self.player_and_kind & 0xf)
    }

    pub fn kind(self) -> WarPlayKind {
        unsafe { mem::transmute(self.player_and_kind >> 4) }
    }
}
