//! Seat-rotated war comparisons with identical rummy strength for every player.
use goat_api::*;
use goat_bot::{
    AdaptSimple, CoverSimple, DuckSimple, PlayTopSimple, Selected, Strategy,
    learned::*,
    rummy_simple, rummy_simulate_seeded,
    war_advanced::{self, Config},
};
use goat_training::Environment;
use rand::SeedableRng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path, time::Instant};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Job {
    seed: u64,
    seeds: usize,
    players: Vec<usize>,
    decks: u8,
    opponents: Vec<String>,
    rummy: String,
    model: Option<String>,
    rummy_config: Option<String>,
    variants: BTreeMap<String, Config>,
}

#[derive(Default)]
struct Game {
    loss: usize,
    moves: usize,
    changed: usize,
    rollouts: usize,
    truncated: usize,
    latency: Vec<f64>,
    rummy_cards: usize,
    rummy_runs: usize,
    received_dreck: bool,
}

fn opponent_style(name: &str, seat: usize, seed: u64) -> &str {
    match name {
        "adapt" | "duck" | "cover" | "top" => name,
        "mixed" => ["duck", "cover", "top", "adapt"][(seat + seed as usize) % 4],
        _ => panic!("unknown war opponent {name}"),
    }
}

fn opponent_action(name: &str, idx: PlayerIdx, view: &War) -> usize {
    match name {
        "adapt" => AdaptSimple.war(idx, view),
        "duck" => DuckSimple.war(idx, view),
        "cover" => CoverSimple.war(idx, view),
        "top" => {
            // A top-only seat normally has no hand, but honor mandatory matches
            // when this helper is used on a state reached by another policy.
            PlayTopSimple
                .war(idx, view)
                .filter(|a| legal_war(idx, view).contains(&action_index(*a)))
                .or_else(|| AdaptSimple.war(idx, view))
        }
        _ => unreachable!(),
    }
    .map(action_index)
    .unwrap_or(WAIT)
}

fn game(
    seed: u64,
    players: usize,
    seat: usize,
    job: &Job,
    config: &Config,
    opponent: &str,
    learned: Option<&Selected>,
) -> Result<Game, String> {
    // One Rayon task owns an entire game, so clearing here makes decisions
    // independent of which worker ran previous games or candidate variants.
    goat_bot::rummy_advanced::clear_proof_cache();
    let mut env = Environment::new(seed, players, job.decks, true, 0)?;
    let mut result = Game::default();
    let mut seen_rummy = false;
    for step in 0..20000 {
        if let Some(goat) = env.goat() {
            result.loss = (goat == seat) as usize;
            return Ok(result);
        }
        let actor = env.player;
        let action = match &env.clients[actor].phase {
            ClientPhase::War(view) => {
                let idx = PlayerIdx(actor as u8);
                if actor == seat {
                    let chosen =
                        war_advanced::search(idx, view, war_advanced::seed(idx, view), config);
                    result.moves += 1;
                    result.changed += (chosen.action != war_advanced::policy(idx, view)) as usize;
                    result.rollouts += chosen.rollouts;
                    result.truncated += chosen.truncated;
                    result.latency.push(chosen.elapsed_ms);
                    chosen.action
                } else {
                    opponent_action(opponent_style(opponent, actor, seed), idx, view)
                }
            }
            ClientPhase::Rummy(view) => {
                if !seen_rummy {
                    seen_rummy = true;
                    let ClientPhase::Rummy(root) = &env.clients[seat].phase else {
                        unreachable!()
                    };
                    result.rummy_cards = root.hands[seat].known.len();
                    result.rummy_runs = root.hands[seat].known.runs().count();
                    result.received_dreck = env.server.events.iter().any(|event| {
                        matches!(event,
                        Event::ReceiveDreck { player, .. } if player.idx() == seat)
                    });
                }
                action_index(match job.rummy.as_str() {
                    "simple" => rummy_simple(view),
                    "advanced" => learned.unwrap().rummy(view),
                    "learned" => decode_action(learned.unwrap().model.choose(view)).unwrap(),
                    "simulate-128" => rummy_simulate_seeded(
                        &mut rand::rngs::StdRng::seed_from_u64(
                            seed.wrapping_add(step as u64 * 65537),
                        ),
                        view,
                        128,
                    ),
                    _ => return Err("unknown rummy policy".into()),
                })
            }
            _ => unreachable!(),
        };
        env.step(action)?;
    }
    Err(format!(
        "game {seed}/{players}/{seat} exceeded decision limit"
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: war_eval JOB.json OUTPUT.json".into());
    }
    let job: Job = serde_json::from_slice(&fs::read(&args[1])?)?;
    if job.seeds == 0
        || job.players.is_empty()
        || job.variants.is_empty()
        || job.opponents.is_empty()
    {
        return Err("empty evaluation".into());
    }
    for config in job.variants.values() {
        config.validate()?;
    }
    for opponent in &job.opponents {
        opponent_style(opponent, 0, 0);
    }
    let learned = if matches!(job.rummy.as_str(), "advanced" | "learned") {
        let mut learned = Selected::default();
        if let Some(path) = &job.model {
            learned.model = std::sync::Arc::new(goat_bot::selected::RummyModel::load(path)?);
        }
        if let Some(path) = &job.rummy_config {
            learned.rummy = serde_json::from_slice(&fs::read(path)?)?;
            learned.rummy.validate()?;
        }
        Some(learned)
    } else {
        None
    };
    if !matches!(
        job.rummy.as_str(),
        "simple" | "learned" | "advanced" | "simulate-128"
    ) {
        return Err("unknown rummy policy".into());
    }
    let mut output = serde_json::json!({"job": job, "complete": false, "results": [],
        "rummy_proof_cache": "reset before each game"});
    if let Some(parent) = Path::new(&args[2])
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let start = Instant::now();
    for (variant, config) in &job.variants {
        for opponent in &job.opponents {
            for &players in &job.players {
                let cell_start = Instant::now();
                let games: Vec<Game> = (0..job.seeds * players)
                    .into_par_iter()
                    .map(|i| {
                        game(
                            job.seed + (i / players) as u64,
                            players,
                            i % players,
                            &job,
                            config,
                            opponent,
                            learned.as_ref(),
                        )
                    })
                    .collect::<Result<_, _>>()?;
                let count = games.len() as f64;
                if config.samples == 0
                    && opponent == "adapt"
                    && games
                        .chunks(players)
                        .any(|seats| seats.iter().map(|g| g.loss).sum::<usize>() != 1)
                {
                    return Err("identical adaptive tables changed outcomes across seat rotations; check determinism".into());
                }
                let losses: usize = games.iter().map(|g| g.loss).sum();
                let clusters: Vec<f64> = games
                    .chunks(players)
                    .map(|g| g.iter().map(|g| g.loss).sum::<usize>() as f64 / players as f64)
                    .collect();
                let mut latency: Vec<_> = games
                    .iter()
                    .flat_map(|g| g.latency.iter().copied())
                    .collect();
                latency.sort_by(f64::total_cmp);
                let percentile = |p: f64| {
                    latency
                        .get(((latency.len().saturating_sub(1)) as f64 * p) as usize)
                        .copied()
                        .unwrap_or(0.)
                };
                let row = serde_json::json!({
                    "variant": variant, "opponent": opponent, "players": players, "games": games.len(),
                    "goats": losses, "goat_rate": losses as f64/count, "cluster_losses": clusters,
                    "moves": games.iter().map(|g|g.moves).sum::<usize>(),
                    "changed": games.iter().map(|g|g.changed).sum::<usize>(),
                    "rollouts": games.iter().map(|g|g.rollouts).sum::<usize>(),
                    "truncated": games.iter().map(|g|g.truncated).sum::<usize>(),
                    "latency_ms_p50": percentile(0.5), "latency_ms_p95": percentile(0.95), "latency_ms_max": percentile(1.),
                    "rummy_cards_mean": games.iter().map(|g|g.rummy_cards).sum::<usize>() as f64/count,
                    "rummy_runs_mean": games.iter().map(|g|g.rummy_runs).sum::<usize>() as f64/count,
                    "dreck_rate": games.iter().filter(|g|g.received_dreck).count() as f64/count,
                    "seconds": cell_start.elapsed().as_secs_f64(), "illegal_actions": 0, "stalled_games": 0,
                });
                let mut display = row.clone();
                display.as_object_mut().unwrap().remove("cluster_losses");
                println!("{display}");
                output["results"].as_array_mut().unwrap().push(row);
                output["seconds"] = start.elapsed().as_secs_f64().into();
                fs::write(&args[2], serde_json::to_vec_pretty(&output)?)?;
            }
        }
    }
    output["complete"] = true.into();
    fs::write(&args[2], serde_json::to_vec_pretty(&output)?)?;
    Ok(())
}
