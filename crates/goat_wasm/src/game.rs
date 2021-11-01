use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};

use goat_api::{
    Card, Cards, ClientGame, ClientPhase, ClientRummyHand, ClientWarHand, PlayerIdx, UserId,
    WarPlay, WarPlayKind, WarTrick,
};

#[derive(Serialize)]
pub struct WasmGame<'a> {
    phase: WasmPhase<'a>,
    players: &'a [UserId],
}

#[derive(Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum WasmPhase<'a> {
    Unstarted,
    #[serde(rename_all = "camelCase")]
    War {
        deck: u8,
        hands: Map<'a, ClientWarHand, WasmHand>,
        won: Map<'a, Cards, u8>,
        curr_trick: WasmWarTrick<'a>,
        prev_trick: Option<WasmWarTrick<'a>>,
    },
    #[serde(rename_all = "camelCase")]
    Rummy {
        hands: Map<'a, ClientRummyHand, WasmHand>,
        trick: WasmRummyTrick<'a>,
        next_player: PlayerIdx,
        trump: Card,
    },
    Complete {
        goat: PlayerIdx,
    },
}

#[derive(Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum WasmHand {
    Visible { cards: Cards },
    Hidden { len: u8 },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmWarTrick<'a> {
    next_player: PlayerIdx,
    plays: Map<'a, WarPlay, WasmWarPlay>,
}

#[derive(Serialize)]
struct WasmWarPlay {
    card: Card,
    player: PlayerIdx,
    kind: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmRummyTrick<'a> {
    num_players: usize,
    plays: &'a [(Card, Card)],
}

impl<'a> WasmGame<'a> {
    pub fn new(game: &'a ClientGame<Option<WarTrick>>) -> Self {
        Self {
            phase: match &game.phase {
                ClientPhase::Unstarted => WasmPhase::Unstarted,
                ClientPhase::War(war) => WasmPhase::War {
                    deck: war.deck.len(),
                    hands: Map::new(&*war.hands, |hand| match hand {
                        ClientWarHand::Visible(cards) => WasmHand::Visible {
                            cards: cards.cards().collect(),
                        },
                        ClientWarHand::Hidden(len) => WasmHand::Hidden { len: *len },
                    }),
                    won: Map::new(&*war.won, |cards| cards.len() as u8),
                    curr_trick: war_trick(&war.trick),
                    prev_trick: war.prev_trick.as_ref().map(war_trick),
                },
                ClientPhase::Rummy(rummy) => WasmPhase::Rummy {
                    hands: Map::new(&*rummy.hands, |hand| {
                        if hand.unknown == 0 {
                            WasmHand::Visible { cards: hand.known }
                        } else {
                            WasmHand::Hidden {
                                len: hand.unknown + hand.known.len() as u8,
                            }
                        }
                    }),
                    trick: WasmRummyTrick {
                        num_players: rummy.trick.num_players(),
                        plays: &*rummy.trick.plays(),
                    },
                    next_player: rummy.next,
                    trump: rummy.trump,
                },
                ClientPhase::Complete(goat) => WasmPhase::Complete { goat: *goat },
            },
            players: &*game.players,
        }
    }
}

fn war_trick(trick: &WarTrick) -> WasmWarTrick {
    WasmWarTrick {
        next_player: trick.next_player(),
        plays: Map::new(trick.plays(), |play| WasmWarPlay {
            card: play.card,
            player: play.player(),
            kind: match play.kind() {
                WarPlayKind::PlayHand => "playHand",
                WarPlayKind::PlayTop => "playTop",
                WarPlayKind::Slough => "slough",
            },
        }),
    }
}

struct Map<'a, T, U> {
    items: &'a [T],
    map: fn(&T) -> U,
}

impl<'a, T, U> Map<'a, T, U> {
    fn new(items: &'a [T], map: fn(&T) -> U) -> Self {
        Self { items, map }
    }
}

impl<'a, T, U> Serialize for Map<'a, T, U>
where
    U: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;
        for item in self.items.iter().map(self.map) {
            seq.serialize_element(&item)?;
        }
        seq.end()
    }
}
