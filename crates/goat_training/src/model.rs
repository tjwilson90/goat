//! Full training checkpoint, including training-only value and war networks.
use goat_bot::{learned::*, selected::Dense};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Clone, Serialize, Deserialize)]
pub struct Model {
    pub schema: u32,
    pub train_war: bool,
    pub trunk: Vec<Dense>,
    pub policy: Dense,
    pub value: Dense,
    #[serde(default)]
    pub war: Option<Box<Model>>,
    #[serde(default)]
    pub candidate: Option<Vec<Dense>>,
    #[serde(default)]
    pub candidate_schema: u32,
    #[serde(default)]
    pub prior: f32,
}

impl Model {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let model: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        model.validate()?;
        Ok(model)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA || self.trunk.is_empty() {
            return Err("unsupported model schema".into());
        }
        let mut width = OBSERVATIONS;
        for layer in &self.trunk {
            if layer.bias.is_empty() || !layer.valid(width, layer.bias.len()) {
                return Err("invalid trunk weights".into());
            }
            width = layer.bias.len();
        }
        if !self.policy.valid(width, ACTIONS) || !self.value.valid(width, 1) {
            return Err("invalid output weights".into());
        }
        if let Some(war) = &self.war {
            war.validate()?;
        }
        if let Some(layers) = &self.candidate {
            if self.candidate_schema != goat_bot::rummy_features::FEATURE_SCHEMA
                || layers.is_empty()
                || !self.prior.is_finite()
            {
                return Err("invalid candidate scorer schema".into());
            }
            let mut width = goat_bot::rummy_features::FEATURES;
            for layer in layers {
                if layer.bias.is_empty() || !layer.valid(width, layer.bias.len()) {
                    return Err("invalid candidate scorer weights".into());
                }
                width = layer.bias.len();
            }
            if width != 1 {
                return Err("candidate scorer must output one score".into());
            }
        }
        Ok(())
    }

    pub fn forward(&self, observation: &[f32]) -> (Vec<f32>, f32) {
        if observation[MAX_PLAYERS * SEAT_FEATURES] > 0.5 {
            if let Some(war) = &self.war {
                return war.forward(observation);
            }
        }
        let mut hidden = observation.to_vec();
        for layer in &self.trunk {
            hidden = layer.forward(&hidden, true);
        }
        let mut scores = self.policy.forward(&hidden, false);
        if let Some(layers) = &self.candidate {
            if observation[MAX_PLAYERS * SEAT_FEATURES + 1] > 0.5 {
                scores.fill(-1e9);
                for (action, features) in goat_bot::rummy_features::candidates(observation) {
                    let mut hidden = features.to_vec();
                    for (i, layer) in layers.iter().enumerate() {
                        hidden = layer.forward(&hidden, i + 1 != layers.len());
                    }
                    scores[action] = hidden[0] + self.prior * features[0];
                }
            }
        }
        (scores, self.value.forward(&hidden, false)[0])
    }

    pub fn choose(&self, observation: &[f32], legal: &[usize]) -> Option<usize> {
        let (scores, _) = self.forward(observation);
        legal
            .iter()
            .copied()
            .filter(|&i| scores[i].is_finite())
            .max_by(|&a, &b| scores[a].total_cmp(&scores[b]).then_with(|| b.cmp(&a)))
    }
}
