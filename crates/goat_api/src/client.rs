use std::collections::HashMap;

use crate::{ClientGame, GameId, GoatError, Response, Slot, UserDb, WarTrick};

pub struct Client<Users, PrevTrick> {
    pub games: HashMap<GameId, ClientGame<PrevTrick>>,
    pub users: Users,
}

impl<Users: UserDb, PrevTrick: Slot<WarTrick>> Client<Users, PrevTrick> {
    pub fn new(users: Users) -> Self {
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
            Response::ForgetGame { game_id } => {
                self.games.remove(&game_id);
            }
            Response::User { user_id, user } => {
                self.users.insert(user_id, user);
            }
            Response::ForgetUser { user_id } => {
                self.users.remove(&user_id);
            }
        }
        Ok(())
    }
}
