use serde::ser::{SerializeSeq, SerializeStruct};
use serde::{Serialize, Serializer};

use goat_api::{
    Card, Cards, ClientDeck, ClientRummyHand, ClientWarHand, PlayerIdx, Rank, RummyTrick,
    ServerWarHand, WarPlay, WarPlayKind, WarTrick,
};

use crate::OneAction;

type ClientGame = goat_api::ClientGame<Option<WarTrick>, OneAction>;
type ClientPhase = goat_api::ClientPhase<Option<WarTrick>, OneAction>;
type WarPhase = goat_api::WarPhase<ClientDeck, ClientWarHand, Option<WarTrick>>;

pub struct Wrapper<T>(pub T);
struct WrapperContext<T, C>(T, C);

impl<'a> Serialize for Wrapper<&'a ClientGame> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = ser.serialize_struct("ClientGame", 2)?;
        ser.serialize_field("phase", &Wrapper(&self.0.phase))?;
        ser.serialize_field("players", &*self.0.players)?;
        ser.end()
    }
}

impl<'a> Serialize for Wrapper<&'a ClientPhase> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            ClientPhase::Unstarted => {
                let mut ser = ser.serialize_struct("ClientPhase", 0)?;
                ser.serialize_field("type", "unstarted")?;
                ser.end()
            }
            ClientPhase::War(war) => {
                let mut ser = ser.serialize_struct("ClientPhase", 6)?;
                ser.serialize_field("type", "war")?;
                ser.serialize_field("deck", &war.deck.len())?;
                ser.serialize_field("hands", &WrapperContext(&*war.hands, war))?;
                ser.serialize_field("won", &Wrapper(&*war.won))?;
                ser.serialize_field("finished", &war.is_finished())?;
                ser.serialize_field("currTrick", &WrapperContext(&war.trick, war.hands.len()))?;
                match &war.prev_trick {
                    Some(trick) => {
                        ser.serialize_field("prevTrick", &WrapperContext(trick, war.hands.len()))?
                    }
                    None => ser.skip_field("prevTrick")?,
                };
                ser.end()
            }
            ClientPhase::Rummy(rummy) => {
                let mut ser = ser.serialize_struct("ClientPhase", 6)?;
                ser.serialize_field("type", "rummy")?;
                ser.serialize_field(
                    "hands",
                    &WrapperContext(&*rummy.hands, (rummy.trump, &rummy.trick)),
                )?;
                ser.serialize_field("trick", &Wrapper(&rummy.trick))?;
                ser.serialize_field("next", &rummy.next)?;
                ser.serialize_field("trump", &rummy.trump)?;
                ser.serialize_field("history", &rummy.history)?;
                ser.end()
            }
            ClientPhase::Goat(goat) => {
                let mut ser = ser.serialize_struct("ClientPhase", 2)?;
                ser.serialize_field("type", "goat")?;
                ser.serialize_field("goat", &goat.goat)?;
                ser.serialize_field("noise", &goat.noise)?;
                ser.end()
            }
        }
    }
}

impl<'a, 'b> Serialize for WrapperContext<&'a [ClientWarHand], &'b WarPhase> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = ser.serialize_seq(Some(self.0.len()))?;
        for (idx, hand) in self.0.iter().enumerate() {
            ser.serialize_element(&WrapperContext(hand, (PlayerIdx(idx as u8), self.1)))?;
        }
        ser.end()
    }
}

impl<'a, 'b> Serialize for WrapperContext<&'a ClientWarHand, (PlayerIdx, &'b WarPhase)> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hand = self.0;
        let (idx, war) = self.1;
        match hand {
            ClientWarHand::Visible(hand) => {
                let mut ser = ser.serialize_struct("ClientWarHand", 2)?;
                ser.serialize_field("type", "visible")?;
                ser.serialize_field("cards", &WrapperContext(hand, (idx, war)))?;
                ser.end()
            }
            ClientWarHand::Hidden(len) => {
                let mut ser = ser.serialize_struct("ClientWarHand", 2)?;
                ser.serialize_field("type", "hidden")?;
                ser.serialize_field("length", &len)?;
                ser.end()
            }
        }
    }
}

impl<'a, 'b> Serialize for WrapperContext<&'a ServerWarHand, (PlayerIdx, &'a WarPhase)> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hand = self.0;
        let (idx, war) = self.1;
        let mut ser = ser.serialize_seq(None)?;
        for card in hand.cards() {
            ser.serialize_element(&WrapperContext(card, (idx, hand, war)))?;
        }
        ser.end()
    }
}

impl<'a> Serialize for WrapperContext<Card, (PlayerIdx, &'a ServerWarHand, &'a WarPhase)> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let card = self.0;
        let (idx, hand, war) = self.1;
        let mut ser = ser.serialize_struct("Card", 3)?;
        ser.serialize_field("card", &card)?;
        let playable = !war.is_finished() && war.trick.check_can_play(idx, hand, card).is_ok();
        ser.serialize_field("playable", &playable)?;
        let sloughable = war.trick.check_can_slough(idx, hand, card).is_ok();
        ser.serialize_field("sloughable", &sloughable)?;
        ser.end()
    }
}

impl<'a> Serialize for Wrapper<&'a [Cards]> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = ser.serialize_seq(None)?;
        for cards in self.0 {
            ser.serialize_element(&cards.len())?;
        }
        ser.end()
    }
}

impl<'a> Serialize for WrapperContext<&'a WarTrick, usize> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let num_players = self.1;
        let mut ser = ser.serialize_struct("WarTrick", 5)?;
        ser.serialize_field("next", &self.0.next_player())?;
        ser.serialize_field("rank", &self.0.rank())?;
        ser.serialize_field("plays", &WrapperContext(self.0.plays(), num_players))?;
        ser.serialize_field("winner", &self.0.winner())?;
        ser.serialize_field("endMask", &self.0.end_mask())?;
        ser.end()
    }
}

impl<'a> Serialize for WrapperContext<&'a [WarPlay], usize> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = ser.serialize_seq(Some(self.0.len()))?;
        if self.0.is_empty() {
            return ser.end();
        }

        let mut copy = WarTrick::new(self.0[0].player(), self.1);
        for play in self.0 {
            ser.serialize_element(&WrapperContext(play, &copy))?;
            if matches!(play.kind(), WarPlayKind::Slough) {
                copy.slough(play.player(), play.card);
            } else {
                copy.play(play.kind(), play.card);
            }
        }
        ser.end()
    }
}

impl<'a, 'b> Serialize for WrapperContext<&'a WarPlay, &'b WarTrick> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let (play, trick) = (self.0, self.1);
        let mut ser = ser.serialize_struct("WarPlay", 4)?;
        ser.serialize_field("card", &play.card)?;
        ser.serialize_field("player", &play.player())?;
        ser.serialize_field(
            "kind",
            match play.kind() {
                WarPlayKind::PlayHand => "playHand",
                WarPlayKind::PlayTop => "playTop",
                WarPlayKind::Slough => "slough",
            },
        )?;
        ser.serialize_field(
            "lead",
            &(play.kind() != WarPlayKind::Slough && trick.rank().is_none()),
        )?;
        ser.end()
    }
}

impl<'a, 'b> Serialize for WrapperContext<&'a [ClientRummyHand], (Card, &'b RummyTrick)> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = ser.serialize_seq(Some(self.0.len()))?;
        for hand in self.0 {
            ser.serialize_element(&WrapperContext(hand, self.1))?;
        }
        ser.end()
    }
}

impl<'a, 'b> Serialize for WrapperContext<&'a ClientRummyHand, (Card, &'b RummyTrick)> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = ser.serialize_struct("ClientRummyHand", 2)?;
        ser.serialize_field("cards", &WrapperContext(self.0.known, self.1))?;
        ser.serialize_field("length", &(self.0.known.len() + self.0.unknown as usize))?;
        ser.end()
    }
}

impl<'a> Serialize for WrapperContext<Cards, (Card, &'a RummyTrick)> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hand = self.0;
        let (trump, trick) = self.1;
        let mut ser = ser.serialize_seq(Some(hand.len()))?;
        let mut run_min = Card::AceSpades;
        for card in hand.into_iter().rev() {
            if card.rank() == Rank::Two || !hand.contains(card.with_rank(card.rank().next_down())) {
                run_min = card;
            }
            ser.serialize_element(&RummyCard {
                card,
                run_min,
                can_play: trick.can_play(card, trump.suit()),
            })?;
        }
        ser.end()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RummyCard {
    card: Card,
    run_min: Card,
    can_play: bool,
}

impl<'a> Serialize for Wrapper<&'a RummyTrick> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = ser.serialize_struct("RummyTrick", 2)?;
        ser.serialize_field("numPlayers", &self.0.num_players())?;
        ser.serialize_field("plays", self.0.plays())?;
        ser.end()
    }
}
