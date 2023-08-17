use std::collections::HashMap;

use js_sys::Array;
use wasm_bindgen::prelude::*;

use goat_api::{Cards, ClientPhase, Response, User, UserId, WarTrick};

use crate::{OneAction, Wrapper};

#[wasm_bindgen]
pub struct Client {
    client: goat_api::Client<HashMap<UserId, User>, Option<WarTrick>, OneAction>,
}

#[wasm_bindgen]
impl Client {
    #[allow(clippy::new_without_default)]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Client {
        Self {
            client: goat_api::Client::new(HashMap::new()),
        }
    }

    pub fn apply(&mut self, response: JsValue) -> Result<(), JsValue> {
        let response: Response = serde_wasm_bindgen::from_value(response)?;
        self.client
            .apply(response)
            .map_err(|e| JsValue::from(format!("Failed to apply response: {}", e)))?;
        Ok(())
    }

    #[wasm_bindgen(js_name = gameIds)]
    pub fn game_ids(&self) -> Result<Array, JsValue> {
        Ok(self
            .client
            .games
            .keys()
            .map(|item| serde_wasm_bindgen::to_value(&item))
            .collect::<Result<_, _>>()?)
    }

    pub fn game(&self, game_id: JsValue) -> Result<JsValue, JsValue> {
        let game_id = serde_wasm_bindgen::from_value(game_id)?;
        match self.client.games.get(&game_id) {
            Some(game) => Ok(serde_wasm_bindgen::to_value(&Wrapper(game))?),
            None => Err(JsValue::from(format!("Unknown game {}", game_id))),
        }
    }

    #[wasm_bindgen(js_name = userIds)]
    pub fn user_ids(&self) -> Result<Array, JsValue> {
        Ok(self
            .client
            .users
            .keys()
            .map(|item| serde_wasm_bindgen::to_value(&item))
            .collect::<Result<_, _>>()?)
    }

    pub fn user(&self, user_id: JsValue) -> Result<JsValue, JsValue> {
        let user_id = serde_wasm_bindgen::from_value(user_id)?;
        match self.client.users.get(&user_id) {
            Some(user) => Ok(serde_wasm_bindgen::to_value(user)?),
            None => Err(JsValue::from(format!("Unknown user {}", user_id))),
        }
    }

    #[wasm_bindgen(js_name = handContainsRange)]
    pub fn hand_contains_range(
        &self,
        game_id: JsValue,
        player_id: JsValue,
        lo: JsValue,
        hi: JsValue,
    ) -> Result<bool, JsValue> {
        let game_id = serde_wasm_bindgen::from_value(game_id)?;
        let player_id: usize = serde_wasm_bindgen::from_value(player_id)?;
        let lo = serde_wasm_bindgen::from_value(lo)?;
        let hi = serde_wasm_bindgen::from_value(hi)?;
        if let Some(game) = self.client.games.get(&game_id) {
            if let ClientPhase::Rummy(rummy) = &game.phase {
                let hand = rummy.hands[player_id].known;
                return Ok(hand.contains_all(Cards::range(lo, hi)));
            }
        }
        Ok(false)
    }
}
