use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use goat_api::{Client, UserId};

#[wasm_bindgen]
pub struct State {
    client: Client<HashMap<UserId, String>>,
}

#[wasm_bindgen]
impl State {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            client: Client::new(HashMap::new()),
        }
    }

    pub fn apply(&mut self, response: &str) -> Result<(), JsValue> {
        let response = serde_json::from_str(response).map_err(|e| JsValue::from(e.to_string()))?;
        self.client
            .apply(response)
            .map_err(|e| JsValue::from(e.to_string()))?;
        Ok(())
    }
}
