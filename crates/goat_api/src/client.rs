use std::collections::HashMap;

use crate::{ClientGame, GameId, GoatError, Response, UserDb};

pub struct Client<U> {
    pub games: HashMap<GameId, ClientGame>,
    pub users: U,
}

impl<U: UserDb> Client<U> {
    pub fn new(users: U) -> Self {
        Self {
            games: HashMap::new(),
            users,
        }
    }

    pub fn apply(&mut self, response: Response) -> Result<(), GoatError> {
        match response {
            Response::Ping => {}
            Response::Replay { game_id, events } => {
                let mut game = ClientGame::new();
                for event in events {
                    game.apply(event)?;
                }
                self.games.insert(game_id, game);
            }
            Response::Game { game_id, event } => match self.games.get_mut(&game_id) {
                Some(game) => game.apply(event)?,
                None => return Err(GoatError::InvalidGame { game_id }),
            },
            Response::Forget { game_id } => {
                self.games.remove(&game_id);
            }
            Response::ChangeName { user_id, name } => {
                self.users.insert(user_id, name);
            }
            Response::Disconnect { user_id } => {
                self.users.remove(&user_id);
            }
        }
        Ok(())
    }
}
