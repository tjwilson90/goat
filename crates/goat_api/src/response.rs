use serde::{Deserialize, Serialize};

use crate::{Event, GameId, User, UserId};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum Response {
    #[serde(rename_all = "camelCase")]
    Ping,
    #[serde(rename_all = "camelCase")]
    Replay { game_id: GameId, events: Vec<Event> },
    #[serde(rename_all = "camelCase")]
    Game { game_id: GameId, event: Event },
    #[serde(rename_all = "camelCase")]
    ForgetGame { game_id: GameId },
    #[serde(rename_all = "camelCase")]
    User { user_id: UserId, user: User },
    #[serde(rename_all = "camelCase")]
    ForgetUser { user_id: UserId },
}
