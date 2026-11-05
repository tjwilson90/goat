"""Imitation warm start, episodic PPO, held-out tournaments, and Rust export.

Run from the repository root with .venv/bin/python training/train.py --help.
The Rust engine owns all rules, observations, and legal masks.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import random
import platform
import subprocess
import time

os.environ.setdefault("RAYON_NUM_THREADS", "6")

import numpy as np
import torch
from torch import nn
import goat_training as engine


class Policy(nn.Module):
    def __init__(self, width=128):
        super().__init__()
        self.width = width
        self.trunk = nn.Sequential(nn.Linear(engine.OBSERVATIONS, width), nn.ReLU(),
                                   nn.Linear(width, width), nn.ReLU())
        self.policy = nn.Linear(width, engine.ACTIONS)
        self.value = nn.Linear(width, 1)
        self.war = None
        nn.init.orthogonal_(self.policy.weight, gain=0.01)
        nn.init.zeros_(self.policy.bias)

    def forward(self, obs, mask):
        hidden = self.trunk(obs)
        logits = self.policy(hidden).masked_fill(~mask, -1e9)
        values = self.value(hidden).squeeze(-1)
        if self.war is not None:
            rows = obs[:, 15 * 111] > 0.5
            if rows.any():
                war_logits, war_values = self.war(obs[rows], mask[rows])
                logits[rows], values[rows] = war_logits, war_values
        return logits, values


class CandidatePolicy(Policy):
    """Shared legal-move scorer with a fixed heuristic prior and learned residual.

    Starts at the simple policy; all legal moves remain available to learning.
    Rust constructs the exact same suit-relative features at deployment.
    """
    def __init__(self, width=64, prior=2.0, feature_schema=1):
        super().__init__(width)
        self.prior = prior
        self.feature_schema = feature_schema
        if feature_schema != 1:
            raise ValueError("only the selected candidate feature schema 1 is supported")
        self.feature_count = engine.CANDIDATE_FEATURES
        self.candidate = nn.Sequential(nn.Linear(self.feature_count, width), nn.ReLU(),
                                       nn.Linear(width, 1))
        nn.init.zeros_(self.candidate[2].weight)
        nn.init.zeros_(self.candidate[2].bias)

    def forward(self, obs, mask):
        logits, values = super().forward(obs, mask)
        indices, packed = engine.candidates(obs.detach().numpy().astype('<f4', copy=False).tobytes(), self.feature_schema)
        if indices:
            indices = torch.tensor(indices, dtype=torch.long)
            features = torch.from_numpy(np.frombuffer(packed, dtype='<f4').copy().reshape(-1, self.feature_count))
            scores = self.candidate(features).squeeze(-1) + self.prior * features[:, 0]
            rummy = obs[:, 15 * 111 + 1] > 0.5
            logits = logits.clone()
            logits[rummy] = -1e9
            logits[indices[:, 0], indices[:, 1]] = scores
            logits = logits.masked_fill(~mask, -1e9)
        return logits, values


def observe(batch):
    observations, masks, teachers, players, phases = batch.observe()
    # Copy immutable Rust-owned byte buffers before handing them to PyTorch.
    obs = np.frombuffer(observations, dtype="<f4").reshape(-1, engine.OBSERVATIONS).copy()
    mask = np.frombuffer(masks, dtype=np.uint8).reshape(-1, engine.ACTIONS).astype(bool)
    return obs, mask, np.asarray(teachers), np.asarray(players), np.frombuffer(phases, dtype=np.uint8)


def permute_suits(obs, mask, teacher, permutation):
    """Return transformed observations/actions and the new-to-original action map."""
    card_order = (13 * np.asarray(permutation)[:, None] + np.arange(13)).reshape(-1)
    order = np.arange(engine.ACTIONS)
    order[1:365] = (1 + 91 * np.asarray(permutation)[:, None] + np.arange(91)).reshape(-1)
    order[365:417] = 365 + card_order
    order[417:469] = 417 + card_order
    inverse = np.argsort(order)
    result = obs.copy()
    seats = result[:, :15 * 111].reshape(-1, 15, 111)
    for offset in [0, 52]:
        seats[:, :, offset:offset + 52] = seats[:, :, offset + card_order]
    global_state = result[:, 15 * 111:]
    for offset in [16, 68, 120, 172, 224]:
        global_state[:, offset:offset + 52] = global_state[:, offset + card_order]
    trick = global_state[:, 276:].reshape(-1, 15, 8)
    trick[:, :, 2:6] = trick[:, :, 2 + np.asarray(permutation)]
    return result, mask[:, order], inverse[teacher], order


def save(model, path, train_war, metadata):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".pt.tmp")
    torch.save({"state": model.state_dict(), "width": model.width, "schema": engine.SCHEMA,
                "architecture": "candidate" if isinstance(model, CandidatePolicy) else "flat",
                "candidate_schema": model.feature_schema if isinstance(model, CandidatePolicy) else 0,
                "prior": getattr(model, "prior", 0.),
                "train_war": train_war, "has_war": model.war is not None, "metadata": metadata}, temporary)
    temporary.replace(path)
    def dense(layer):
        return {"weight": layer.weight.detach().cpu().tolist(), "bias": layer.bias.detach().cpu().tolist()}
    def export(network):
        return {"schema": engine.SCHEMA, "train_war": train_war,
                "trunk": [dense(network.trunk[0]), dense(network.trunk[2])],
                "policy": dense(network.policy), "value": dense(network.value),
                "war": export(network.war) if network.war is not None else None,
                "candidate": [dense(network.candidate[0]), dense(network.candidate[2])] if isinstance(network, CandidatePolicy) else None,
                "candidate_schema": network.feature_schema if isinstance(network, CandidatePolicy) else 0,
                "prior": getattr(network, "prior", 0.)}
    exported = export(model)
    temporary = path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(exported, separators=(",", ":")) + "\n")
    temporary.replace(path.with_suffix(".json"))


def load(path):
    checkpoint = torch.load(path, map_location="cpu", weights_only=True)
    if checkpoint["schema"] != engine.SCHEMA:
        raise ValueError("checkpoint schema does not match environment")
    if checkpoint.get("architecture") == "candidate":
        if checkpoint.get("candidate_schema") not in [1, 2]:
            raise ValueError("candidate feature schema does not match environment")
        model = CandidatePolicy(checkpoint["width"], checkpoint["prior"], checkpoint['candidate_schema'])
    else:
        model = Policy(checkpoint["width"])
    if checkpoint.get("has_war"):
        model.war = Policy(checkpoint["width"])
    model.load_state_dict(checkpoint["state"])
    return model, checkpoint


def collect(model, *, games, seed, player_counts, deck_counts, train_war, war_style,
            imitation=False, historical=None, phase="joint", opponents="league", suit_augmentation=False,
            balanced=False, dagger=False, teacher_style="simple"):
    """Complete episodes: undiscounted -1/0 terminal returns for each acting seat.

    No transition is bootstrapped from another player's value. Historical and
    heuristic seats are excluded from PPO updates. Forced moves are excluded
    from policy training but still simulated normally.
    """
    rng = np.random.default_rng(seed)
    counts = rng.choice(player_counts, games).tolist()
    decks = rng.choice(deck_counts, games).tolist()
    seeds = [seed + i for i in range(games)]
    batch = engine.Batch(seeds, counts, decks, train_war, war_style)
    # 0=current, 1=heuristic, 2=frozen checkpoint. At least one learner per game.
    roster = np.zeros((games, 15), dtype=np.int64)
    if not imitation and opponents == "league":
        roster = rng.choice(3, (games, 15), p=[0.6, 0.2, 0.2])
        if historical is None:
            roster[roster == 2] = 1
        for i, n in enumerate(counts):
            roster[i, rng.integers(n)] = 0
    elif not imitation and opponents == "simple":
        roster.fill(1)
        for i, n in enumerate(counts):
            roster[i, rng.integers(n)] = 0
    obs_rows, mask_rows, action_rows, log_rows, value_rows, game_rows, seat_rows = [], [], [], [], [], [], []
    done = np.zeros(games, dtype=bool)
    goats = np.full(games, -1)
    decisions = 0
    start = time.perf_counter()
    while not done.all():
        obs, mask, teacher, players, phases = observe(batch)
        if teacher_style != "simple":
            teacher = np.asarray(batch.baseline(teacher_style,
                [seed * 100003 + decisions + i for i in range(games)], (~done).tolist()))
        action_order = np.arange(engine.ACTIONS)
        if suit_augmentation:
            obs, mask, teacher, action_order = permute_suits(obs, mask, teacher, rng.permutation(4))
        active = np.flatnonzero(~done)
        actions = teacher.copy()
        selected = active
        if not imitation:
            learner = roster[np.arange(games), players] == 0
            learn_phase = np.ones(games, dtype=bool)
            if phase == "rummy":
                learn_phase = phases == 1
            elif phase == "war":
                learn_phase = phases == 0
            # Frozen rummy continuation during war training; fixed heuristics for
            # war during rummy-only training are already handled by Rust.
            selected = np.flatnonzero(~done & learner & learn_phase)
            with torch.no_grad():
                logits, values = model(torch.from_numpy(obs), torch.from_numpy(mask))
                distribution = torch.distributions.Categorical(logits=logits)
                sampled = distribution.sample()
                log_probs = distribution.log_prob(sampled).numpy()
                actions = sampled.numpy()
                value_estimates = values.numpy()
                actions[roster[np.arange(games), players] == 1] = teacher[roster[np.arange(games), players] == 1]
                if historical is not None:
                    frozen = (roster[np.arange(games), players] == 2) | (learner & ~learn_phase)
                    if frozen.any():
                        old_logits, _ = historical(torch.from_numpy(obs[frozen]), torch.from_numpy(mask[frozen]))
                        actions[frozen] = torch.distributions.Categorical(logits=old_logits).sample().numpy()
        else:
            log_probs = np.zeros(games, dtype=np.float32)
            value_estimates = np.zeros(games, dtype=np.float32)
            if phase == "war":
                rummy_rows = np.flatnonzero(~done & (phases == 1))
                if rummy_rows.size:
                    with torch.no_grad():
                        continuation_logits, _ = model(torch.from_numpy(obs[rummy_rows]), torch.from_numpy(mask[rummy_rows]))
                    actions[rummy_rows] = continuation_logits.argmax(-1).numpy()
            if phase != "joint":
                selected = selected[phases[selected] == (phase == "rummy")]
        # Forced decisions carry no policy learning signal.
        selected = selected[mask[selected].sum(axis=1) > 1]
        if selected.size:
            obs_rows.append(obs[selected])
            mask_rows.append(mask[selected])
            action_rows.append(actions[selected].copy())
            log_rows.append(log_probs[selected].copy())
            value_rows.append(value_estimates[selected].copy())
            game_rows.append(selected.copy())
            seat_rows.append(players[selected].copy())
        if dagger:
            # Labels above are the teacher's; execute the learner to expose its
            # own state distribution (DAgger), without mislabeling PPO samples.
            with torch.no_grad():
                actions = model(torch.from_numpy(obs), torch.from_numpy(mask))[0].argmax(-1).numpy()
        actions[done] = 472
        goats = np.asarray(batch.step(action_order[actions].tolist()))
        done = goats >= 0
        decisions += len(active)
    if not obs_rows:
        raise RuntimeError("no trainable decisions in collection")
    game_ids = np.concatenate(game_rows)
    seats = np.concatenate(seat_rows)
    returns = -(goats[game_ids] == seats).astype(np.float32)
    data = {"obs": torch.from_numpy(np.concatenate(obs_rows)),
            "mask": torch.from_numpy(np.concatenate(mask_rows)),
            "action": torch.from_numpy(np.concatenate(action_rows)).long(),
            "old_log": torch.from_numpy(np.concatenate(log_rows)),
            "old_value": torch.from_numpy(np.concatenate(value_rows)),
            "returns": torch.from_numpy(returns)}
    weights = np.ones(len(returns), dtype=np.float32)
    if balanced:
        configurations = np.array(counts)[game_ids] * 10 + np.array(decks)[game_ids]
        for configuration in np.unique(configurations):
            rows = configurations == configuration
            weights[rows] = len(returns) / (len(np.unique(configurations)) * rows.sum())
    data["weight"] = torch.from_numpy(weights)
    return data, {"games": games, "decisions": decisions, "samples": len(returns),
                  "collection_seconds": round(time.perf_counter() - start, 3)}


def optimize(model, optimizer, data, *, imitation, epochs, minibatch, entropy=0.01, anchor=None):
    n = len(data["action"])
    all_weights = data.get("weight", torch.ones(n))
    advantages = data["returns"] - data["old_value"]
    advantages = (advantages - advantages.mean()) / (advantages.std() + 1e-8)
    metrics = []
    for _ in range(epochs):
        for ids in torch.randperm(n).split(minibatch):
            weights = all_weights[ids]
            def mean(values):
                return (values * weights).mean()
            logits, values = model(data["obs"][ids], data["mask"][ids])
            distribution = torch.distributions.Categorical(logits=logits)
            log_prob = distribution.log_prob(data["action"][ids])
            if imitation:
                policy_loss = -mean(log_prob)
            else:
                ratio = (log_prob - data["old_log"][ids]).exp()
                policy_loss = -mean(torch.minimum(ratio * advantages[ids], ratio.clamp(0.8, 1.2) * advantages[ids]))
            value_loss = mean((values - data["returns"][ids]).square())
            entropy_value = mean(distribution.entropy())
            loss = policy_loss + 0.5 * value_loss - (0 if imitation else entropy) * entropy_value
            if anchor is not None and not imitation:
                # A small trust anchor slows catastrophic loss of the warm start.
                with torch.no_grad():
                    target, _ = anchor(data["obs"][ids], data["mask"][ids])
                loss = loss + 0.02 * mean(torch.distributions.kl_divergence(
                    torch.distributions.Categorical(logits=target), distribution))
            optimizer.zero_grad(set_to_none=True)
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 0.5)
            optimizer.step()
            metrics.append((float(policy_loss.detach()), float(value_loss.detach()),
                            float(entropy_value.detach()), float((logits.argmax(-1) == data["action"][ids]).float().mean())))
    return dict(zip(["policy_loss", "value_loss", "entropy", "action_accuracy"], np.mean(metrics, axis=0).tolist()))


def tournament(model, *, seeds, players, decks, opponent="simple", train_war=False,
               war_style=0, candidate="model", batch_size=128, native=None, search_samples=0, endgame=False,
               advanced=None, opponent_advanced=None):
    """One candidate seat, every rotation per seed; uncertainty clustered by deal."""
    losses = []
    lengths = []
    times = []
    search_times = []
    search_changes = search_proofs = 0
    rollout_count = truncated_count = proof_nodes = 0
    move_latencies = []
    start = time.perf_counter()
    games = [(seed, seat) for seed in seeds for seat in range(players)]
    for offset in range(0, len(games), batch_size):
        chunk = games[offset:offset + batch_size]
        n = len(chunk)
        batch = engine.Batch([s for s, _ in chunk], [players] * n, [decks] * n, train_war, war_style)
        seats = np.array([s for _, s in chunk])
        done = np.zeros(n, dtype=bool)
        steps = np.zeros(n, dtype=np.int64)
        goats = np.full(n, -1)
        tick = 0
        while not done.all():
            obs, mask, teachers, acting, phases = observe(batch)
            actions = teachers.copy()
            # Opponent randomness is derived from deal/seat/tick, never model RNG.
            random_seeds = [(s * 1000003 + tick * 97 + int(acting[i])) % (2**64) for i, (s, _) in enumerate(chunk)]
            if opponent == "mixed":
                for style_id, style in enumerate(["simple", "rollout", "simulate-128", "search-16"]):
                    enabled = ~done & (acting != seats) & (((acting + np.array([s for s,_ in chunk])) % 4) == style_id)
                    base = np.asarray(batch.baseline(style, random_seeds, enabled.tolist()))
                    actions[enabled] = base[enabled]
            elif opponent != "simple":
                actions = np.asarray(batch.baseline(opponent, random_seeds, (~done & (acting != seats)).tolist()))
            if opponent_advanced is not None:
                enabled = ~done & (acting != seats) & (phases == 1)
                priors = teachers.copy()
                result = batch.advanced(random_seeds, priors.tolist(), enabled.tolist(), json.dumps(opponent_advanced))
                actions[enabled] = [result[i][0] for i in np.flatnonzero(enabled)]
            chosen = np.flatnonzero(~done & (acting == seats))
            if candidate == "model" and chosen.size:
                t = time.perf_counter()
                if native is not None:
                    raw, _ = native.predict(obs[chosen].astype("<f4").tobytes())
                    logits = np.frombuffer(raw, dtype="<f4").reshape(-1, engine.ACTIONS)
                    actions[chosen] = np.where(mask[chosen], logits, -1e9).argmax(-1)
                else:
                    with torch.no_grad():
                        logits, _ = model(torch.from_numpy(obs[chosen]), torch.from_numpy(mask[chosen]))
                        actions[chosen] = logits.argmax(-1).numpy()
                times.append((time.perf_counter() - t) / len(chosen))
            elif candidate != "model":
                baseline = teachers if candidate == "simple" else np.asarray(batch.baseline(candidate, random_seeds, (~done & (acting == seats)).tolist()))
                actions[chosen] = baseline[chosen]
            if search_samples or endgame:
                enabled = ~done & (acting == seats) & (phases == 1)
                search_start = time.perf_counter()
                improved = batch.search(random_seeds, actions.tolist(), enabled.tolist(), search_samples, not search_samples)
                if enabled.any():
                    search_times.append((time.perf_counter() - search_start) / int(enabled.sum()))
                search_changes += sum(improved[i][0] != actions[i] for i in np.flatnonzero(enabled))
                search_proofs += sum(improved[i][2] for i in np.flatnonzero(enabled))
                actions[enabled] = [improved[i][0] for i in np.flatnonzero(enabled)]
            if advanced is not None:
                enabled = ~done & (acting == seats) & (phases == 1)
                improved = batch.advanced(random_seeds, actions.tolist(), enabled.tolist(), json.dumps(advanced))
                rows = [improved[i] for i in np.flatnonzero(enabled)]
                search_changes += sum(improved[i][0] != actions[i] for i in np.flatnonzero(enabled))
                search_proofs += sum(r[2] for r in rows)
                rollout_count += sum(r[3] for r in rows)
                truncated_count += sum(r[4] for r in rows)
                proof_nodes += sum(r[5] for r in rows)
                move_latencies.extend(r[6] for r in rows)
                actions[enabled] = [r[0] for r in rows]
            actions[done] = 472
            steps += ~done
            goats = np.asarray(batch.step(actions.tolist()))
            done = goats >= 0
            tick += 1
        losses.extend((goats == seats).astype(float).tolist())
        lengths.extend(steps.tolist())
    clusters = np.asarray(losses).reshape(-1, players).mean(axis=1)
    rate = float(clusters.mean())
    se = float(clusters.std(ddof=1) / math.sqrt(len(clusters))) if len(clusters) > 1 else float("nan")
    interval = [max(0, rate - 1.96 * se), min(1, rate + 1.96 * se)]
    # No observed losses does not establish zero risk. Use a conservative
    # exact all-zero/all-one bound over independent deal clusters at boundaries.
    if rate == 0:
        interval = [0., 1. - .025 ** (1. / len(clusters))]
    elif rate == 1:
        interval = [.025 ** (1. / len(clusters)), 1.]
    return {"candidate": candidate, "opponent": opponent, "players": players, "decks": decks,
            "games": len(games), "independent_seeds": len(seeds), "goat_rate": rate,
            "ci95": interval,
            "mean_decisions": float(np.mean(lengths)), "seconds": round(time.perf_counter() - start, 3),
            "amortized_batch_inference_ms_p50": float(np.median(times) * 1000) if times else None,
            "amortized_batch_inference_ms_p95": float(np.percentile(times, 95) * 1000) if times else None,
            "search_samples": search_samples, "endgame": endgame,
            "search_changes": int(search_changes), "search_proofs": int(search_proofs),
            "advanced": advanced, "rollouts": rollout_count, "truncated_rollouts": truncated_count,
            "proof_nodes": proof_nodes,
            "search_ms_p50": float(np.median(move_latencies)) if move_latencies else None,
            "search_ms_p95": float(np.percentile(move_latencies,95)) if move_latencies else None,
            "search_ms_max": float(max(move_latencies)) if move_latencies else None,
            "amortized_batch_search_ms_p95": float(np.percentile(search_times, 95) * 1000) if search_times else None,
            "cluster_losses": clusters.tolist(), "illegal_actions": 0, "stalled_games": 0}


def parity(model, path):
    native = engine.NativeModel(str(Path(path).with_suffix(".json")))
    maximum = 0.0
    phases_seen = set()
    for war in [False, True]:
        batch = engine.Batch(list(range(40, 56)), [3, 4, 5, 6] * 4, [1, 2] * 8, war, 0)
        for _ in range(80):
            obs, mask, teacher, _, phases = observe(batch)
            phases_seen.update(phases.tolist())
            with torch.no_grad():
                logits, value = model(torch.from_numpy(obs), torch.ones_like(torch.from_numpy(mask)))
            raw_logits, raw_value = native.predict(obs.astype("<f4").tobytes())
            actual = np.frombuffer(raw_logits, dtype="<f4").reshape(-1, engine.ACTIONS)
            actual_value = np.frombuffer(raw_value, dtype="<f4")
            np.testing.assert_allclose(actual, logits.numpy(), atol=3e-5, rtol=3e-5)
            np.testing.assert_allclose(actual_value, value.numpy(), atol=3e-5, rtol=3e-5)
            expected_action = np.where(mask, logits.numpy(), -1e9).argmax(-1)
            actual_action = np.where(mask, actual, -1e9).argmax(-1)
            np.testing.assert_array_equal(actual_action, expected_action)
            maximum = max(maximum, float(np.abs(actual - logits.numpy()).max()))
            batch.step(teacher.tolist())
    assert {0, 1}.issubset(phases_seen)
    return {"positions": 2560, "max_logit_difference": maximum, "actions_identical": True}


def benchmark_native(path, train_war, decisions=500):
    native = engine.NativeModel(str(Path(path).with_suffix(".json")))
    seed = 3_000_000
    batch = engine.Batch([seed], [4], [2], train_war, 0)
    durations = []
    for _ in range(decisions):
        observations, _, teachers, _, _ = batch.observe()
        start = time.perf_counter()
        native.predict(observations)
        durations.append(1000 * (time.perf_counter() - start))
        if batch.step(teachers)[0] >= 0:
            seed += 1
            batch = engine.Batch([seed], [4], [2], train_war, 0)
    return {"decisions": decisions, "p50_ms": float(np.median(durations)),
            "p95_ms": float(np.percentile(durations, 95)), "max_ms": max(durations)}


def train(args):
    torch.manual_seed(args.seed)
    np.random.seed(args.seed)
    random.seed(args.seed)
    model, previous = load(args.resume) if args.resume else (
        CandidatePolicy(args.width) if args.architecture == "candidate" else Policy(args.width), {})
    train_war = args.phase != "rummy"
    if args.phase == "war" and not args.resume:
        raise ValueError("war training requires --resume with a rummy checkpoint")
    if train_war and model.war is None:
        model.war = Policy(model.width)
    if args.phase == "war":
        for component in [model.trunk, model.policy, model.value]:
            for parameter in component.parameters():
                parameter.requires_grad_(False)
    out = Path(args.output)
    if (out / "config.json").exists():
        raise ValueError(f"{out} already contains a run; use a new --output directory")
    out.mkdir(parents=True, exist_ok=True)
    config = vars(args).copy()
    config["runtime"] = {"torch": str(torch.__version__), "numpy": np.__version__,
                         "python": platform.python_version(), "platform": platform.platform(),
                         "schema": engine.SCHEMA,
                         "git_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()}
    config["source_sha256"] = {str(path): hashlib.sha256(path.read_bytes()).hexdigest()
                               for path in [Path(__file__), Path("crates/goat_training/src/lib.rs"),
                                            Path("crates/goat_bot/src/learned.rs"),
                                            Path("crates/goat_bot/src/rummy_features.rs"),
                                            Path("crates/goat_bot/src/rummy_advanced.rs"),
                                            Path("crates/goat_training/src/model.rs")]}
    (out / "config.json").write_text(json.dumps(config, indent=2) + "\n")
    optimizer = torch.optim.Adam(model.parameters(), lr=args.learning_rate)
    log = (out / "metrics.jsonl").open("a", buffering=1)
    def record(row):
        print(json.dumps(row), flush=True)
        log.write(json.dumps(row) + "\n")
    seed_cursor = args.seed
    for iteration in range(args.imitation_iterations + args.dagger_iterations):
        data, stats = collect(model, games=args.games_per_iteration, seed=seed_cursor,
                              player_counts=args.players, deck_counts=args.decks, train_war=train_war,
                              war_style=args.war_style, imitation=True, phase=args.phase,
                              suit_augmentation=args.suit_augmentation, balanced=args.balanced,
                              dagger=iteration >= args.imitation_iterations, teacher_style=args.teacher)
        seed_cursor += args.games_per_iteration
        metrics = optimize(model, optimizer, data, imitation=True, epochs=args.imitation_epochs, minibatch=args.minibatch)
        record({"stage": "dagger" if iteration >= args.imitation_iterations else "imitation",
                "iteration": iteration + 1, **stats, **metrics})
    save(model, out / "imitation.pt", train_war, config)
    anchor = copy.deepcopy(model).eval()
    history = [copy.deepcopy(model).eval()]
    for group in optimizer.param_groups:
        group["lr"] = args.ppo_learning_rate
    best = float("inf")
    validation_seeds = list(range(args.validation_seed, args.validation_seed + args.validation_seeds))
    def validate(iteration):
        nonlocal best
        results = [tournament(model, seeds=validation_seeds, players=n, decks=d,
                              train_war=train_war, war_style=args.war_style)
                   for n in args.players for d in args.decks]
        # Scale by random-seat risk so configurations with more seats matter.
        score = float(np.mean([r["goat_rate"] * r["players"] for r in results]))
        record({"stage": "validation", "iteration": iteration, "normalized_goat_rate": score,
                "results": [{k: v for k, v in r.items() if k != "cluster_losses"} for r in results]})
        if score < best:
            best = score
            save(model, out / "best.pt", train_war, {**config, "iteration": iteration, "validation_score": best})
    validate(0)
    for iteration in range(args.ppo_iterations):
        opponent = history[iteration % len(history)]
        data, stats = collect(model, games=args.games_per_iteration, seed=seed_cursor,
                              player_counts=args.players, deck_counts=args.decks, train_war=train_war,
                              war_style=args.war_style, historical=opponent, phase=args.phase,
                              suit_augmentation=args.suit_augmentation, balanced=args.balanced,
                              opponents=args.training_opponents)
        seed_cursor += args.games_per_iteration
        metrics = optimize(model, optimizer, data, imitation=False, epochs=args.ppo_epochs,
                           minibatch=args.minibatch, entropy=args.entropy, anchor=anchor)
        record({"stage": "ppo", "iteration": iteration + 1, **stats, **metrics})
        if (iteration + 1) % 5 == 0:
            history.append(copy.deepcopy(model).eval())
            history = history[-8:]
        if (iteration + 1) % args.validate_every == 0 or iteration + 1 == args.ppo_iterations:
            validate(iteration + 1)
            save(model, out / "last.pt", train_war, {**config, "iteration": iteration + 1})
    selected, _ = load(out / "best.pt")
    record({"stage": "export_parity", **parity(selected, out / "best.pt")})
    log.close()


def evaluate(args):
    model, checkpoint = load(args.model)
    seeds = list(range(args.seed, args.seed + args.seeds))
    native = engine.NativeModel(str(Path(args.model).with_suffix(".json"))) if args.native else None
    results = []
    for n in args.players:
        for d in args.decks:
            for opponent in args.opponents:
                result = tournament(model, seeds=seeds, players=n, decks=d, opponent=opponent,
                                    train_war=checkpoint["train_war"], war_style=args.war_style, native=native,
                                    search_samples=args.search_samples, endgame=args.endgame)
                control = tournament(model, seeds=seeds, players=n, decks=d, opponent=opponent,
                                     train_war=checkpoint["train_war"], war_style=args.war_style, candidate="simple")
                if args.search_control:
                    search_control = tournament(model, seeds=seeds, players=n, decks=d, opponent=opponent,
                        train_war=checkpoint["train_war"], war_style=args.war_style, candidate="simple",
                        search_samples=args.search_samples, endgame=args.endgame)
                    delta_search = np.array(result['cluster_losses']) - np.array(search_control.pop('cluster_losses'))
                    se_search = float(delta_search.std(ddof=1) / math.sqrt(len(delta_search)))
                    result['search_control'] = search_control
                    result['paired_difference_vs_search_control'] = float(delta_search.mean())
                    result['paired_ci95_vs_search_control'] = [float(delta_search.mean()) - 1.96 * se_search,
                                                               float(delta_search.mean()) + 1.96 * se_search]
                differences = np.array(result.pop("cluster_losses")) - np.array(control.pop("cluster_losses"))
                se = float(differences.std(ddof=1) / math.sqrt(len(differences)))
                delta = float(differences.mean())
                result["control"] = control
                result["paired_goat_rate_difference"] = delta
                result["paired_ci95"] = [delta - 1.96 * se, delta + 1.96 * se]
                results.append(result)
                print(json.dumps(result), flush=True)
    report = {"model": args.model, "seed_start": args.seed, "native_inference": args.native,
              "search_samples": args.search_samples, "endgame": args.endgame,
              "model_sha256": hashlib.sha256(Path(args.model).with_suffix(".json").read_bytes()).hexdigest(),
              "results": results, "export_parity": parity(model, args.model),
              "single_decision_rust_inference": benchmark_native(args.model, checkpoint["train_war"])}
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    Path(args.output).write_text(json.dumps(report, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--threads", type=int, default=4)
    sub = parser.add_subparsers(dest="command", required=True)
    train_parser = sub.add_parser("train")
    train_parser.add_argument("--output", default="training/runs/rummy")
    train_parser.add_argument("--resume")
    train_parser.add_argument("--phase", choices=["rummy", "war", "joint"], default="rummy")
    train_parser.add_argument("--seed", type=int, default=1000)
    train_parser.add_argument("--players", type=int, nargs="+", default=[3, 4, 5, 6])
    train_parser.add_argument("--decks", type=int, nargs="+", default=[1, 2])
    train_parser.add_argument("--war-style", type=int, choices=range(6), default=0)
    train_parser.add_argument("--width", type=int, default=128)
    train_parser.add_argument("--architecture", choices=["flat", "candidate"], default="flat")
    train_parser.add_argument("--games-per-iteration", type=int, default=128)
    train_parser.add_argument("--imitation-iterations", type=int, default=20)
    train_parser.add_argument("--dagger-iterations", type=int, default=0)
    train_parser.add_argument("--teacher", default="simple", choices=["simple", "search-16", "search-64"])
    train_parser.add_argument("--balanced", action="store_true", help="Equal loss weight per player/deck configuration")
    train_parser.add_argument("--training-opponents", choices=["league", "simple"], default="league")
    train_parser.add_argument("--imitation-epochs", type=int, default=4)
    train_parser.add_argument("--ppo-iterations", type=int, default=40)
    train_parser.add_argument("--ppo-epochs", type=int, default=3)
    train_parser.add_argument("--minibatch", type=int, default=512)
    train_parser.add_argument("--learning-rate", type=float, default=1e-3)
    train_parser.add_argument("--ppo-learning-rate", type=float, default=1e-4)
    train_parser.add_argument("--entropy", type=float, default=0.01)
    train_parser.add_argument("--suit-augmentation", action="store_true")
    train_parser.add_argument("--validate-every", type=int, default=10)
    train_parser.add_argument("--validation-seeds", type=int, default=50)
    train_parser.add_argument("--validation-seed", type=int, default=1_000_000)
    eval_parser = sub.add_parser("evaluate")
    eval_parser.add_argument("--model", required=True)
    eval_parser.add_argument("--output", default="training/runs/evaluation.json")
    eval_parser.add_argument("--players", type=int, nargs="+", default=[3, 4, 5, 6])
    eval_parser.add_argument("--decks", type=int, nargs="+", default=[1, 2])
    eval_parser.add_argument("--opponents", nargs="+", choices=["simple", "random", "rollout", "simulate-128", "simulate", "search-16", "search-64", "endgame"], default=["simple", "rollout"])
    eval_parser.add_argument("--search-samples", type=int, default=0)
    eval_parser.add_argument("--endgame", action="store_true")
    eval_parser.add_argument("--search-control", action="store_true", help="Also compare simple plus the identical search budget")
    eval_parser.add_argument("--seed", type=int, default=2_000_000)
    eval_parser.add_argument("--seeds", type=int, default=500)
    eval_parser.add_argument("--war-style", type=int, choices=range(6), default=0)
    eval_parser.add_argument("--native", action="store_true")
    args = parser.parse_args()
    if args.threads < 1 or any(n < 3 or n > 15 for n in args.players) or any(d < 1 or d > 3 for d in args.decks):
        parser.error("expected positive threads, 3..15 players, and 1..3 decks")
    if args.seed < 0:
        parser.error("seed must be nonnegative")
    if args.command == "train":
        if min(args.games_per_iteration, args.minibatch, args.width, args.validate_every) < 1 or args.validation_seeds < 2:
            parser.error("batch sizes/width/validation interval must be positive; use at least two validation seeds")
        if min(args.imitation_iterations, args.dagger_iterations, args.ppo_iterations) < 0 or min(args.imitation_epochs, args.ppo_epochs) < 1:
            parser.error("iterations must be nonnegative and epochs must be positive")
    elif args.seeds < 2:
        parser.error("use at least two evaluation seeds for confidence intervals")
    torch.set_num_threads(args.threads)
    if args.command == "train":
        train(args)
    else:
        evaluate(args)


if __name__ == "__main__":
    main()
