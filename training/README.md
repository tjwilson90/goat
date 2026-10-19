# Training and evaluating the default Goat bot

This pipeline uses the real Rust game engine, player-specific redacted client
states, a masked neural policy, and a value head trained on the final goat.
It supports imitation, rummy-only PPO, war PPO with a frozen rummy continuation,
and joint PPO. Games run without network traffic or real-time sleeps.

See [the initial training results](RESULTS.md) for completed runs, measured
strength, limitations, model artifacts, and exact reproduction commands.

See [the one-deck experiments](ONE_DECK.md) for the candidate scorer,
search-guided training, completed comparisons, and the improved one-deck bot.

See [the war evaluation](WAR.md) for the selected full-game war search and
comparisons with fixed rummy strength.

## Setup

From the repository root, with Rust, Python 3.12+ and `uv` installed:

```sh
uv venv .venv
uv pip install --python .venv/bin/python -r training/requirements.txt
VIRTUAL_ENV="$PWD/.venv" .venv/bin/maturin develop --release --manifest-path crates/goat_training/Cargo.toml --features python
```

If your environment requires a corporate CA bundle, point `CARGO_HTTP_CAINFO`
at that bundle. In a restricted filesystem, set `UV_CACHE_DIR` to a writable
directory. Neither setting is needed by the game or trained model.

## Train and evaluate

```sh
.venv/bin/python training/train.py train --output training/runs/rummy \
  --players 3 4 5 6 --decks 1 2 --suit-augmentation

.venv/bin/python training/train.py train --phase war \
  --resume training/runs/rummy/best.pt --output training/runs/war \
  --seed 100000 --players 3 4 5 6 --decks 1 2 --suit-augmentation

.venv/bin/python training/train.py train --phase joint \
  --resume training/runs/war/best.pt --output training/runs/joint \
  --seed 200000 --imitation-iterations 0 --suit-augmentation

.venv/bin/python training/train.py evaluate \
  --model training/runs/rummy/best.pt --players 3 4 5 6 --decks 1 2 \
  --seeds 2500 --opponents simple rollout --output training/runs/evaluation.json

.venv/bin/python training/train.py evaluate \
  --model training/runs/rummy/best.pt --players 3 4 --decks 1 2 \
  --seeds 100 --opponents simulate-128 --output training/runs/search-evaluation.json
```

`--threads` is a global option before `train` or `evaluate` (default 4).
`RAYON_NUM_THREADS` controls Rust workers (default 6). CPU inference is the
default; a GPU is not required. `--native` evaluates the exported Rust model.
`--war-style` selects the fixed war baseline: 0 adaptive, 1 duck, 2 cover,
3 play-top, 4 a deterministic mixture, 5 the selected full-game war search.
Search can also supply war imitation labels through `--phase war --war-style 5`. Do not change this between candidates
when measuring a rummy improvement.

Every training run writes configuration, JSONL metrics, an imitation checkpoint,
the latest PPO checkpoint, and the checkpoint selected by validation. `.pt`
files are PyTorch checkpoints; matching `.json` files are directly loadable by
Rust. The simulator/observation schema is versioned. Checkpoints resume weights,
not optimizer/RNG state, so resuming starts a new recorded experiment. Use a new
output directory and disjoint training seeds when continuing training.

Training defaults to seeds starting at 1,000; validation uses 1,000,000 onwards;
final evaluation uses 2,000,000 onwards. Keep these ranges disjoint. Validation
selects the lowest average goat rate normalized by `1 / player_count`. Final
evaluation must not be used to select checkpoints or tune hyperparameters.

## Run the default bots

```sh
cargo run --release -p goat_server
```

Every server bot now uses `Selected::default()`: conservative 64-world war
search and the selected learned rummy scorer with public-information rollouts
and cached endgame proofs. Its compact weights are bundled in
[`crates/goat_bot/models/rummy.json`](../crates/goat_bot/models/rummy.json), so a
fresh checkout needs neither Python nor files under `training/runs` to serve.
Two- and three-deck games retain the original `AdaptSimulate` strategy.

The `GOAT_MODEL`, `GOAT_SEARCH_CONFIG`, `GOAT_SEARCH_SAMPLES`, `GOAT_ENDGAME`, and
`GOAT_WAR_CONFIG` serving switches have been removed. Search budgets live in
the Rust defaults; the matching files in `training/configs` are for evaluation.
The discarded target/shape war heuristics, opponent portfolios, tree search,
heuristic evolution, schema-2 scorer, and duplicate older rummy search have been
removed. Full checkpoint/value-network handling lives in the training crate;
production embeds only the selected action scorer.

The training workflows below remain available to retrain the schema-1 scorer.
Search teachers and the `search-16`/`search-64` evaluation baselines now use the
selected public-information search. Historical reports describe the versions
used at the time; removed experimental variants are not reproducible commands
against the cleaned-up code.

## What is being learned

- The sole reward is -1 for the goat and 0 for every other player. PPO uses
  complete episodes and per-seat Monte Carlo returns, with no step penalty or
  cross-player value bootstrapping. Forced actions are executed but omitted
  from policy updates.
- A shared policy plays multiple seats against a league of heuristic seats
  and frozen historical checkpoints. Only current-policy decisions contribute
  PPO gradients. The policy uses clipped PPO updates, entropy regularization,
  and a small KL anchor to the initial imitation policy.
- Each phase uses two 128-unit ReLU layers with policy and value heads. War
  training updates a separate network so the rummy continuation remains fixed.
  Joint training updates both. The value predicts expected reward (negative
  goat risk), rather than a calibrated probability constrained to [0, 1].
- The 2,061 observation features contain 15 relative seat slots, card
  multiplicities, public winnings, unknown counts, eligibility, pickup counters,
  phase/trick information, and recent trick order. Padding supports all 3–15
  player / 1–3 deck configurations. Training on 3–6 players does not establish
  strength outside that distribution.
- There are 473 action labels: pickup, all 364 same-suit intervals, 52 hand
  plays, 52 sloughs, draw, play-top, acknowledge, and scheduler wait. Masks derive
  solely from the acting player's observation. The engine still validates
  each executed action.
- Optional suit augmentation transforms observations, masks, labels, and
  sampled actions together, including during PPO collection. Seat order is
  relative to the acting player; clockwise order is retained.

## Interpretation and limitations

The tournament rotates one candidate through every seat for each held-out deal.
It reports goat rates, seed-clustered confidence intervals, and paired
differences against a heuristic candidate using the same deals and opponents.
A negative paired difference is an improvement. Identical-policy self-play
goat rate is not a measure of progress. Timing measurements in the tournament
are amortized over inference batches, not network latency in the live server.
Illegal actions or excessive episode lengths fail the run instead of being
quietly counted as a draw or assigned a fabricated winner.

Opponent names have distinct meanings:

| Name | Rummy policy |
| --- | --- |
| `simple` | Existing `rummy_simple` heuristic |
| `random` | Uniform over every legal action |
| `rollout` | Existing heuristic-biased `rummy_random`; no search |
| `simulate-128` | Existing determinization/rollout algorithm with 128 seeded rollouts per move |
| `simulate` | Actual existing three-second `rummy_simulate` bot |

The three-second bot is substantially more expensive to evaluate and retains
its original time-dependent randomness. Use `simulate-128` for reproducible
search comparisons, and do not describe beating it as beating the three-second
bot. Both inherit the existing search's limited root-action coverage and
full-information simulated continuations. Learned policies see only their own
redacted views.

War uses a seeded round-robin scheduler. All players receive opportunities for
out-of-turn draws/sloughs; waits are internal scheduler decisions, and mandatory
plays/acknowledgments cannot wait indefinitely. This preserves legal game
actions but approximates live timing. It does not train reaction speeds or
network-delay tactics. The feedforward observation summarizes long war ties
and does not retain a full opponent-action history. A recurrent policy and richer learned beliefs are future extensions.
The one-deck extension adds redacted rollout search and bounded endgame proofs;
see [ONE_DECK.md](ONE_DECK.md).

## Verification

```sh
cargo test -p goat_training -p goat_bot -p goat_api
.venv/bin/python -m unittest discover -s training -v
cargo test -p goat_server
```

Tests cover deterministic replays, card conservation, client/server agreement,
privacy, exhaustive rummy legal masks, legal war decisions, suit transformations,
repeatable rollouts, frozen continuations, malformed model rejection, and
PyTorch/Rust inference parity. Export parity also runs after training and final
evaluation on both game phases.

## Selected one-deck search

[ADVANCED.md](ADVANCED.md) records the rummy selection and held-out results.
`training/advanced.py evaluate --variants prior selected` compares the scorer
alone with the selected rummy search. `training/war.py` compares adaptive and
selected war play using the bundled rummy scorer by default when `--rummy
advanced` is specified. Both retain paired deal/seat evaluation.
