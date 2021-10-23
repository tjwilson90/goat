use serde::{Deserialize, Serialize};

use crate::{Event, GameId, UserId};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Response {
    Ping,
    Replay { game_id: GameId, events: Vec<Event> },
    Game { game_id: GameId, event: Event },
    Forget { game_id: GameId },
    ChangeName { user_id: UserId, name: String },
    Disconnect { user_id: UserId },
}
