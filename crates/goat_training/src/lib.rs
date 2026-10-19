//! Deterministic headless games. Only redacted client views reach a policy.
use goat_api::*;
use goat_bot::{learned::*, *};
pub mod model;
#[cfg(feature = "python")]
use model::Model;

#[derive(Clone)]
pub struct Environment {
    pub server: ServerGame,
    pub clients: Vec<ClientGame<(), Cards>>,
    pub player: usize,
    pub train_war: bool,
    cursor: usize,
    war_style: usize,
    actions: usize,
}

impl Environment {
    pub fn new(
        seed: u64,
        players: usize,
        decks: u8,
        train_war: bool,
        war_style: usize,
    ) -> Result<Self, String> {
        if !(3..=15).contains(&players) || !(1..=3).contains(&decks) || war_style > 5 {
            return Err("expected 3..15 players, 1..3 decks, and war style 0..5".into());
        }
        let mut env = Self {
            server: ServerGame::with_seed(seed),
            clients: vec![ClientGame::default(); players],
            player: 0,
            train_war,
            cursor: (seed as usize) % players,
            war_style,
            actions: 0,
        };
        for i in 0..players {
            let user_id = UserId(RandId::from_hash(&[i as u8; 16]));
            let start = env.server.events.len();
            env.server
                .apply(user_id, Action::Join { user_id })
                .map_err(|e| e.to_string())?;
            env.broadcast(start)?;
        }
        env.apply(0, Action::Start { num_decks: decks })?;
        env.advance()?;
        Ok(env)
    }

    fn broadcast(&mut self, start: usize) -> Result<(), String> {
        for event in &self.server.events[start..] {
            for (idx, client) in self.clients.iter_mut().enumerate() {
                client
                    .apply(event.redact(Some(PlayerIdx(idx as u8))))
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    fn apply(&mut self, player: usize, action: Action) -> Result<(), String> {
        self.actions += 1;
        if self.actions > 20000 {
            return Err(format!("game {} exceeded action limit", self.server.seed));
        }
        let start = self.server.events.len();
        self.server
            .apply(self.server.players[player], action)
            .map_err(|e| {
                format!(
                    "seed {}, player {player}, {action:?}: {e}",
                    self.server.seed
                )
            })?;
        self.broadcast(start)
    }

    fn advance(&mut self) -> Result<(), String> {
        loop {
            match &self.server.phase {
                ServerPhase::Goat(_) => return Ok(()),
                ServerPhase::Rummy(rummy) => {
                    self.player = rummy.next.idx();
                    return Ok(());
                }
                ServerPhase::War(_) => {}
                _ => return Err("game not started".into()),
            }
            let idx = self.cursor;
            self.cursor = (self.cursor + 1) % self.clients.len();
            let ClientPhase::War(war) = &self.clients[idx].phase else {
                unreachable!()
            };
            let legal = legal_war(PlayerIdx(idx as u8), war);
            if self.train_war && legal != [WAIT] {
                self.player = idx;
                return Ok(());
            }
            if !self.train_war {
                let action = self.war_teacher(idx, war);
                if let Some(action) = action {
                    // Fail on simulator/baseline divergence, rather than silently altering rules.
                    self.apply(idx, action)?;
                }
            }
        }
    }

    fn war_teacher(&self, idx: usize, war: &War) -> Option<Action> {
        let player = PlayerIdx(idx as u8);
        let style = if self.war_style == 4 {
            (idx + self.server.seed as usize) % 4
        } else {
            self.war_style
        };
        match style {
            1 => war_duck(player, war),
            2 => war_cover(player, war),
            3 => war_play_top(player, war),
            5 => decode_action(
                war_advanced::search(
                    player,
                    war,
                    war_advanced::seed(player, war),
                    &war_advanced::Config::default(),
                )
                .action,
            ),
            _ => AdaptSimple.war(player, war),
        }
    }

    pub fn observation(&self) -> (Vec<f32>, Vec<usize>, usize, u8) {
        match &self.clients[self.player].phase {
            ClientPhase::War(war) => (
                observe_war(PlayerIdx(self.player as u8), war),
                legal_war(PlayerIdx(self.player as u8), war),
                self.war_teacher(self.player, war)
                    .map(action_index)
                    .unwrap_or(WAIT),
                0,
            ),
            ClientPhase::Rummy(rummy) => (
                observe_rummy(rummy),
                legal_rummy(rummy),
                action_index(rummy_simple(rummy)),
                1,
            ),
            _ => (vec![0.; OBSERVATIONS], vec![WAIT], WAIT, 2),
        }
    }

    pub fn goat(&self) -> Option<usize> {
        match &self.server.phase {
            ServerPhase::Goat(goat) => Some(goat.goat.idx()),
            _ => None,
        }
    }

    pub fn step(&mut self, action: usize) -> Result<(), String> {
        if self.goat().is_some() {
            return Ok(());
        }
        let legal = match &self.clients[self.player].phase {
            ClientPhase::War(war) => legal_war(PlayerIdx(self.player as u8), war),
            ClientPhase::Rummy(rummy) => legal_rummy(rummy),
            _ => unreachable!(),
        };
        if !legal.contains(&action) {
            return Err(format!("illegal policy action {action}"));
        }
        if let Some(action) = decode_action(action) {
            self.apply(self.player, action)?;
        }
        self.advance()
    }

    pub fn baseline(&self, name: &str, seed: u64) -> Result<usize, String> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let (_, legal, teacher, _) = self.observation();
        match name {
            "simple" => Ok(teacher),
            "search-16" | "search-64" | "endgame" => match &self.clients[self.player].phase {
                ClientPhase::Rummy(rummy) => {
                    if name == "endgame" {
                        Ok(rummy_advanced::proved_safe_action(rummy, teacher, 20_000)
                            .unwrap_or(teacher))
                    } else {
                        Ok(rummy_advanced::search(
                            rummy,
                            seed,
                            teacher,
                            &rummy_advanced::Config {
                                samples: if name == "search-16" { 16 } else { 64 },
                                max_rollouts: 4096,
                                ..Default::default()
                            },
                        )
                        .action)
                    }
                }
                _ => Ok(teacher),
            },
            "random" => Ok(legal[rng.gen_range(0..legal.len())]),
            "rollout" => match &self.clients[self.player].phase {
                ClientPhase::Rummy(rummy) => Ok(action_index(rummy_random(&mut rng, rummy))),
                _ => Ok(teacher),
            },
            "simulate" => match &self.clients[self.player].phase {
                ClientPhase::Rummy(rummy) => Ok(action_index(rummy_simulate(rummy))),
                _ => Ok(teacher),
            },
            "simulate-128" => match &self.clients[self.player].phase {
                ClientPhase::Rummy(rummy) => {
                    Ok(action_index(rummy_simulate_seeded(&mut rng, rummy, 128)))
                }
                _ => Ok(teacher),
            },
            _ => Err(format!("unknown baseline {name}")),
        }
    }
}

#[cfg(feature = "python")]
mod python {
    use super::*;
    use pyo3::{exceptions::PyValueError, prelude::*, types::PyBytes};
    use rayon::prelude::*;

    fn error(message: String) -> PyErr {
        PyValueError::new_err(message)
    }

    #[pyclass]
    struct Batch {
        envs: Vec<Environment>,
    }

    #[pymethods]
    impl Batch {
        #[new]
        fn new(
            seeds: Vec<u64>,
            players: Vec<usize>,
            decks: Vec<u8>,
            train_war: bool,
            war_style: usize,
        ) -> PyResult<Self> {
            if seeds.is_empty() || seeds.len() != players.len() || seeds.len() != decks.len() {
                return Err(error(
                    "batch arrays must have equal, nonzero lengths".into(),
                ));
            }
            let envs = seeds
                .par_iter()
                .zip(&players)
                .zip(&decks)
                .map(|((&seed, &n), &d)| Environment::new(seed, n, d, train_war, war_style))
                .collect::<Result<Vec<_>, _>>()
                .map_err(error)?;
            Ok(Self { envs })
        }

        fn observe<'py>(
            &self,
            py: Python<'py>,
        ) -> (
            Bound<'py, PyBytes>,
            Bound<'py, PyBytes>,
            Vec<usize>,
            Vec<usize>,
            Vec<u8>,
        ) {
            let rows: Vec<_> = self.envs.par_iter().map(Environment::observation).collect();
            let mut obs = Vec::with_capacity(rows.len() * OBSERVATIONS * 4);
            let mut masks = vec![0u8; rows.len() * ACTIONS];
            let mut teachers = Vec::new();
            let mut phases = Vec::new();
            for (i, (row, legal, teacher, phase)) in rows.into_iter().enumerate() {
                for value in row {
                    obs.extend_from_slice(&value.to_le_bytes());
                }
                for action in legal {
                    masks[i * ACTIONS + action] = 1;
                }
                teachers.push(teacher);
                phases.push(phase);
            }
            (
                PyBytes::new(py, &obs),
                PyBytes::new(py, &masks),
                teachers,
                self.envs.iter().map(|e| e.player).collect(),
                phases,
            )
        }

        fn step(&mut self, py: Python<'_>, actions: Vec<usize>) -> PyResult<Vec<i32>> {
            if actions.len() != self.envs.len() {
                return Err(error("action batch length mismatch".into()));
            }
            py.detach(|| {
                self.envs
                    .par_iter_mut()
                    .zip(actions)
                    .map(|(env, action)| {
                        env.step(action)?;
                        Ok(env.goat().map_or(-1, |g| g as i32))
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .map_err(error)
        }

        #[pyo3(signature = (name, seeds, enabled=None))]
        fn baseline(
            &self,
            py: Python<'_>,
            name: &str,
            seeds: Vec<u64>,
            enabled: Option<Vec<bool>>,
        ) -> PyResult<Vec<usize>> {
            if seeds.len() != self.envs.len() {
                return Err(error("seed batch length mismatch".into()));
            }
            let enabled = enabled.unwrap_or_else(|| vec![true; self.envs.len()]);
            if enabled.len() != self.envs.len() {
                return Err(error("enabled batch length mismatch".into()));
            }
            py.detach(|| {
                self.envs
                    .par_iter()
                    .zip(seeds)
                    .zip(enabled)
                    .map(|((env, seed), enabled)| {
                        if env.goat().is_some() || !enabled {
                            Ok(WAIT)
                        } else {
                            env.baseline(name, seed)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(error)
        }

        #[pyo3(signature = (seeds, preferred, enabled, config))]
        fn advanced(
            &self,
            py: Python<'_>,
            seeds: Vec<u64>,
            preferred: Vec<usize>,
            enabled: Vec<bool>,
            config: &str,
        ) -> PyResult<
            Vec<(
                usize,
                Vec<(usize, f32, usize)>,
                bool,
                usize,
                usize,
                usize,
                f64,
            )>,
        > {
            let cfg: rummy_advanced::Config =
                serde_json::from_str(config).map_err(|e| error(e.to_string()))?;
            cfg.validate().map_err(error)?;
            let n = self.envs.len();
            if seeds.len() != n || preferred.len() != n || enabled.len() != n {
                return Err(error("advanced batch length mismatch".into()));
            }
            py.detach(|| {
                self.envs
                    .par_iter()
                    .enumerate()
                    .map(|(i, env)| {
                        if !enabled[i] || env.goat().is_some() {
                            return Ok((preferred[i], vec![], false, 0, 0, 0, 0.));
                        }
                        let ClientPhase::Rummy(view) = &env.clients[env.player].phase else {
                            return Ok((preferred[i], vec![], false, 0, 0, 0, 0.));
                        };
                        if !legal_rummy(view).contains(&preferred[i]) {
                            return Err("illegal advanced search prior".to_string());
                        }
                        let result = rummy_advanced::search(view, seeds[i], preferred[i], &cfg);
                        Ok((
                            result.action,
                            result.estimates,
                            result.exact,
                            result.rollouts,
                            result.truncated,
                            result.nodes,
                            result.elapsed_ms,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .map_err(error)
        }

        fn search(
            &self,
            py: Python<'_>,
            seeds: Vec<u64>,
            preferred: Vec<usize>,
            enabled: Vec<bool>,
            samples: usize,
            endgame_only: bool,
        ) -> PyResult<Vec<(usize, Vec<(usize, f32)>, bool, usize)>> {
            let n = self.envs.len();
            if seeds.len() != n || preferred.len() != n || enabled.len() != n {
                return Err(error("search batch length mismatch".into()));
            }
            py.detach(|| {
                self.envs
                    .par_iter()
                    .enumerate()
                    .map(|(i, env)| {
                        if !enabled[i] || env.goat().is_some() {
                            return Ok((preferred[i], vec![], false, 0));
                        }
                        let ClientPhase::Rummy(view) = &env.clients[env.player].phase else {
                            return Ok((preferred[i], vec![], false, 0));
                        };
                        if !legal_rummy(view).contains(&preferred[i]) {
                            return Err("illegal search prior".to_string());
                        }
                        if endgame_only {
                            let proven =
                                rummy_advanced::proved_safe_action(view, preferred[i], 20_000);
                            return Ok((
                                proven.unwrap_or(preferred[i]),
                                vec![],
                                proven.is_some(),
                                0,
                            ));
                        }
                        let config = rummy_advanced::Config {
                            samples,
                            max_rollouts: 65536,
                            ..Default::default()
                        };
                        config.validate()?;
                        let result = rummy_advanced::search(view, seeds[i], preferred[i], &config);
                        Ok((
                            result.action,
                            result
                                .estimates
                                .into_iter()
                                .map(|(a, p, _)| (a, p))
                                .collect(),
                            result.exact,
                            result.rollouts,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .map_err(error)
        }
    }

    #[pyfunction]
    #[pyo3(signature = (observations, schema=1))]
    fn candidates<'py>(
        py: Python<'py>,
        observations: &[u8],
        schema: u32,
    ) -> PyResult<(Vec<(usize, usize)>, Bound<'py, PyBytes>)> {
        if observations.len() % (OBSERVATIONS * 4) != 0 || schema != rummy_features::FEATURE_SCHEMA
        {
            return Err(error("invalid observations".into()));
        }
        let rows: Vec<_> = observations
            .par_chunks(OBSERVATIONS * 4)
            .map(|bytes| {
                let obs: Vec<_> = bytes
                    .chunks_exact(4)
                    .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                    .collect();
                rummy_features::candidates(&obs)
            })
            .collect();
        let mut indices = Vec::new();
        let mut features = Vec::new();
        for (i, row) in rows.into_iter().enumerate() {
            for (action, values) in row {
                indices.push((i, action));
                features.extend(values.iter().flat_map(|v| v.to_le_bytes()));
            }
        }
        Ok((indices, PyBytes::new(py, &features)))
    }

    #[pyclass]
    struct NativeModel {
        model: Model,
    }

    #[pymethods]
    impl NativeModel {
        #[new]
        fn new(path: &str) -> PyResult<Self> {
            Ok(Self {
                model: Model::load(path).map_err(error)?,
            })
        }

        fn predict<'py>(
            &self,
            py: Python<'py>,
            observations: &[u8],
        ) -> PyResult<(Bound<'py, PyBytes>, Bound<'py, PyBytes>)> {
            if observations.len() % (OBSERVATIONS * 4) != 0 {
                return Err(error("invalid observations".into()));
            }
            let rows: Vec<_> = observations
                .par_chunks(OBSERVATIONS * 4)
                .map(|bytes| {
                    let obs: Vec<_> = bytes
                        .chunks_exact(4)
                        .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                        .collect();
                    self.model.forward(&obs)
                })
                .collect();
            let logits: Vec<_> = rows
                .iter()
                .flat_map(|(row, _)| row.iter().flat_map(|v| v.to_le_bytes()))
                .collect();
            let values: Vec<_> = rows.iter().flat_map(|(_, v)| v.to_le_bytes()).collect();
            Ok((PyBytes::new(py, &logits), PyBytes::new(py, &values)))
        }
    }

    #[pymodule]
    fn goat_training(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add_class::<Batch>()?;
        m.add_class::<NativeModel>()?;
        m.add_function(wrap_pyfunction!(candidates, m)?)?;
        m.add("CANDIDATE_FEATURES", rummy_features::FEATURES)?;
        m.add("CANDIDATE_SCHEMA", rummy_features::FEATURE_SCHEMA)?;
        m.add("OBSERVATIONS", OBSERVATIONS)?;
        m.add("ACTIONS", ACTIONS)?;
        m.add("SCHEMA", SCHEMA)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
