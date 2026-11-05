//! The selected one-deck policies, with weights bundled into the binary.
use std::{
    path::Path,
    sync::{Arc, OnceLock},
};

use goat_api::*;
use serde::{Deserialize, Serialize};

use crate::{
    AdaptSimulate, Strategy, learned::*, rummy_advanced, rummy_features, rummy_simple, war_advanced,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Dense {
    pub weight: Vec<Vec<f32>>,
    pub bias: Vec<f32>,
}

impl Dense {
    pub fn valid(&self, inputs: usize, outputs: usize) -> bool {
        self.bias.len() == outputs
            && self.weight.len() == outputs
            && self
                .weight
                .iter()
                .all(|row| row.len() == inputs && row.iter().all(|v| v.is_finite()))
            && self.bias.iter().all(|v| v.is_finite())
    }

    pub fn forward(&self, inputs: &[f32], relu: bool) -> Vec<f32> {
        self.weight
            .iter()
            .zip(&self.bias)
            .map(|(row, bias)| {
                let value = row
                    .iter()
                    .zip(inputs)
                    .fold(*bias, |acc, (w, x)| acc + w * x);
                if relu { value.max(0.) } else { value }
            })
            .collect()
    }
}

/// Only the trained action scorer is needed at inference. The training value
/// head and observation trunk are deliberately absent from the bundled asset.
#[derive(Clone, Serialize, Deserialize)]
pub struct RummyModel {
    pub candidate_schema: u32,
    pub candidate: Vec<Dense>,
    pub prior: f32,
}

impl RummyModel {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        Self::from_json(&std::fs::read(path).map_err(|e| e.to_string())?)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let model: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        model.validate()?;
        Ok(model)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.candidate_schema != rummy_features::FEATURE_SCHEMA
            || self.candidate.is_empty()
            || !self.prior.is_finite()
        {
            return Err("invalid rummy scorer schema".into());
        }
        let mut width = rummy_features::FEATURES;
        for layer in &self.candidate {
            if layer.bias.is_empty() || !layer.valid(width, layer.bias.len()) {
                return Err("invalid rummy scorer weights".into());
            }
            width = layer.bias.len();
        }
        if width != 1 {
            return Err("rummy scorer must output one score".into());
        }
        Ok(())
    }

    pub fn choose(&self, view: &Rummy) -> usize {
        // Use the same observation/feature path as the evaluated checkpoint.
        rummy_features::candidates(&observe_rummy(view))
            .into_iter()
            .map(|(action, features)| {
                let mut hidden = features.to_vec();
                for (i, layer) in self.candidate.iter().enumerate() {
                    hidden = layer.forward(&hidden, i + 1 != self.candidate.len());
                }
                (action, hidden[0] + self.prior * features[0])
            })
            .filter(|(_, score)| score.is_finite())
            .max_by(|(a, x), (b, y)| x.total_cmp(y).then_with(|| b.cmp(a)))
            .map_or_else(|| action_index(rummy_simple(view)), |(action, _)| action)
    }
}

#[derive(Clone)]
pub struct Selected {
    pub model: Arc<RummyModel>,
    pub war: war_advanced::Config,
    pub rummy: rummy_advanced::Config,
}

impl Default for Selected {
    fn default() -> Self {
        static MODEL: OnceLock<Arc<RummyModel>> = OnceLock::new();
        Self {
            model: MODEL
                .get_or_init(|| {
                    Arc::new(
                        RummyModel::from_json(include_bytes!("../models/rummy.json"))
                            .expect("invalid bundled rummy scorer"),
                    )
                })
                .clone(),
            war: war_advanced::Config::default(),
            rummy: rummy_advanced::Config::default(),
        }
    }
}

impl Strategy for Selected {
    fn war(&self, idx: PlayerIdx, war: &War) -> Option<Action> {
        decode_action(
            war_advanced::search(idx, war, war_advanced::seed(idx, war), &self.war).action,
        )
    }

    fn rummy(&self, view: &Rummy) -> Action {
        let cards =
            1 + view.history.len() + view.hands.iter().map(|hand| hand.len()).sum::<usize>();
        if cards != 52 {
            // The selected policies were validated for one deck. Preserve the
            // original server strategy for multi-deck games.
            return AdaptSimulate.rummy(view);
        }
        let preferred = self.model.choose(view);
        decode_action(
            rummy_advanced::search(view, rummy_advanced::seed(view), preferred, &self.rummy).action,
        )
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_scorer_rejects_incompatible_or_damaged_weights() {
        let model = Selected::default().model;
        model.validate().unwrap();
        let mut invalid = (*model).clone();
        invalid.candidate_schema += 1;
        assert!(invalid.validate().is_err());
        let mut invalid = (*model).clone();
        invalid.candidate[0].weight[0].pop();
        assert!(invalid.validate().is_err());
        let mut invalid = (*model).clone();
        invalid.candidate[0].weight[0][0] = f32::NAN;
        assert!(invalid.validate().is_err());
        assert!(RummyModel::from_json(b"{}").is_err());
    }
}
