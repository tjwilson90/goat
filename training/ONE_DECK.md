> Historical experiment report. For current default serving and supported
> training commands, see [README.md](README.md). Experimental serving flags
> and superseded policies described below have been removed.

# One-deck rummy experiments

The subsequent implementation and experiments are in [ADVANCED.md](ADVANCED.md).
This report retains the original bot and measurements as a benchmark.

The one-deck work adds a shared candidate-action scorer, search-guided imitation
and DAgger-style learner-state imitation, balanced training losses, configurable validation seeds, move
failure diagnostics, and information-respecting rollout search. War stays on
`AdaptSimple` throughout these experiments.

The selected artifact is `runs/one-deck-v2/best.pt` (PyTorch) and
`runs/one-deck-v2/best.json` (Rust). `selection.json` records validation-only
selection, the model digest, source digests, and the final test split. Artifacts
under `runs/` are local and ignored by git.

## Training comparisons

Five runs completed **80,640 training games**, 6,991,354 simulated decisions,
and 3,707,598 policy samples. Each validation comparison used 200 reserved
seeds per player count and every seat rotation. Scores are goat rate divided
by the symmetric simple-bot risk, averaged over 3–6 players; lower is better
and the simple reference is 1.

| Experiment | Training games | Best validation score |
| --- | ---: | ---: |
| Fresh flat one-deck model | 30,720 | 1.2000 |
| Fine-tune original flat model | 20,480 | 1.1313 |
| Shared scorer with heuristic prior | 8,960 | 1.0000 |
| Search-guided scorer, first seed | 10,240 | 0.8900 |
| Search-guided scorer, second seed | 10,240 | **0.8875** |

Specialization alone did not fix the regression. The prior removed imitation
errors, and search-guided labels produced a learned improvement. The selected
second-seed checkpoint had completed 20 expert-play and 20 learner-play
iterations (5,120 games); its subsequent PPO did not improve validation.

An independent search ablation on the same validation split gave scores of
0.8913 for simple plus endgame solving, 0.1087 for simple plus 16-sample
rollouts, and 0.1025 for the first learned scorer plus 16-sample rollouts.
64 samples gave 0.0937 for that scorer, but 16 was selected as the practical
budget before final testing. Most of the hybrid gain comes from online search.

The stratified diagnostic queried 741 disagreements on old-model trajectories.
Lead/run selection was common, whereas pickup-versus-play disagreements were
rare. Under matched simple continuations, the old model's average loss
exceeded the teacher's by 1.8–5.1 percentage points across these diagnostic
sets. These conditional, deliberately stratified samples are not an unbiased
estimate of the causes of tournament losses.

## Fresh held-out results

Final evaluation used 2,000 untouched seeds per player count, starting at
7000000, with all seat rotations: **36,000 candidate games per policy variant**.
All candidates and controls used the same adaptive war policy. Inference was
from the exported Rust model. Lower goat rate is better.

| Players | Simple | Learned policy alone | Learned + 16-sample search | Hybrid 95% CI |
| --- | ---: | ---: | ---: | --- |
| 3 | 33.33% | 33.15% | **8.67%** | [8.02%, 9.31%] |
| 4 | 25.00% | 23.61% | **2.25%** | [1.93%, 2.57%] |
| 5 | 20.00% | 17.87% | **1.53%** | [1.29%, 1.77%] |
| 6 | 16.67% | 14.12% | **0.97%** | [0.79%, 1.15%] |

The standalone learned policy improves 4–6 players with paired 95% confidence
intervals excluding zero. Its three-player difference is inconclusive. The
hybrid improves every tested player count against simple opponents, reducing
goat risk by 74–94%. Intervals cluster seat rotations by deal seed.

**Search supplies most of the hybrid gain.** Simple plus identical search had
goat rates of 8.80%, 2.26%, 1.51%, 1.01% for 3–6 players.
The learned prior makes only small differences once full online search is
available; its incremental differences are inconclusive for 4–6 players.
Do not attribute the full hybrid improvement to machine learning.

Against the older 128-rollout search opponents, a separate final test used
500 seeds per configuration starting at 7100000 (3,500 candidate games):

| Players | Simple candidate | Learned + search | Paired difference, percentage points (95% CI) |
| --- | ---: | ---: | --- |
| 3 | 23.20% | 22.07% | -1.13 [-3.19, +0.92] |
| 4 | 21.70% | 19.35% | -2.35 [-4.57, -0.13] |

The four-player improvement excludes zero; the three-player difference does
not. Exploratory validation of the first training seed also showed that adding
simple-policy rollouts could hurt its 5–6 player performance against search
opponents. This is evidence of opponent-model sensitivity, not general search
superiority. Those exploratory results are in
`runs/one-deck-search-opponents-validation.json`. The actual three-second bot
and human opponents have not been benchmarked.

## Verification and latency

- All **30 Rust tests and six Python tests** passed, including the actual
  asynchronous bot runtime with candidate scoring and search.
- Rust and PyTorch chose identical actions on 2,560 parity positions; maximum
  logit difference was `1.91e-06`.
- No illegal actions or action-limit failures occurred in final tournaments.
- Across 1,000 single-game decisions, native model plus search took **0.269 ms median**,
  **2.006 ms p95**, and 9.252 ms maximum on this machine.
  This Python harness calls Rust inference/search and excludes observation
  construction, networking, and deliberate bot pacing. It is an observed
  sample distribution, not a worst-case search deadline.
- The release server loads the selected JSON with `GOAT_SEARCH_SAMPLES=16`.
  A three-bot live game completed through war, dreck redistribution, rummy,
  and the goat event in **172.46 seconds**, including normal bot pacing.
  The running local server is <http://127.0.0.1:9402>.

Detailed artifacts: `evaluation-policy.json`, `evaluation-hybrid.json`,
`evaluation-search-opponents.json`, `latency.json`, `live-smoke.json`, and `selection.json` under
`runs/one-deck-v2/`.

## What changed

The candidate network scores each legal action using the same 80 features and
64-unit hidden layer. Features describe the remaining hand and its runs,
trump conservation, immediate exit, trick completion, connected pickup size,
and the next/previous active opponents' known cards and hand sizes. A fixed
heuristic logit prior of 2 initializes greedy behavior to `rummy_simple`; a
learned residual can override it. No legal move is pruned. Feature schema 1 is
validated separately from the existing observation schema. Python and Rust
share feature construction, with independent neural implementations checked
for export parity.

Training supports:

- One-deck-only collection and checkpoint selection, with optional loss
  weighting to give each player/deck configuration equal total weight.
- Suit augmentation of observations, labels, masks, and actions. The heuristic
  prior retains the simple bot's arbitrary suit tie-breaking; the scorer's
  other features use suits relative to the proposed move and trump.
- Search imitation, followed by DAgger-style updates: query the search teacher while
  executing the learner's greedy moves to collect its own visited states.
- Existing complete-episode PPO, historical opponent league, and an optional
  curriculum with one learner against simple opponents.

Online search considers **every legal move**, using the same sampled hidden
hands for every alternative. Each simulated player gets its own private hand
and a redacted view of the others. The root view cannot distinguish its own
public and private cards, so its initial hand is conservatively hidden from
other simulated players; subsequent public events update all views. Hidden
hands are sampled uniformly subject to known ownership and card counts;
opponent behavior history is not used to infer a richer belief distribution.
Continuations use the simple policy and run to a terminal outcome, or until
the root exits. Truncations are excluded from targets. Fully known positions
need only one sample because their continuations are deterministic.

Two-player positions with at most 12 remaining hand cards also use a bounded
minimax proof search (20,000 nodes, depth 192). A sole unknown opponent hand is
inferred by card accounting. The solver preserves pickup counters and returns
a move only when it proves the root can avoid becoming goat; an exhausted
budget is unknown. Multiplayer rollouts do not treat opponents as a coalition.
The learned value head is still trained, but is not used to truncate search:
these experiments use actual terminal rollout targets.

## Reproduce training

Follow the environment setup in [README.md](README.md). All commands run from
the repository root. Use new output directories when repeating experiments.

Fresh specialization:

```sh
RAYON_NUM_THREADS=3 .venv/bin/python training/train.py --threads 2 train \
  --output training/runs/one-deck-fresh --players 3 4 5 6 --decks 1 \
  --seed 5000000 --validation-seed 6000000 --validation-seeds 200 \
  --games-per-iteration 256 --imitation-iterations 40 --ppo-iterations 80 \
  --validate-every 10 --suit-augmentation --balanced
```

Fine-tuning used the same options with `--resume training/runs/rummy-v1/best.pt`,
`--imitation-iterations 0`, `--seed 5100000`, and output `one-deck-finetune`.
The candidate/prior ablation used `--architecture candidate --width 64`,
128 games per iteration, 10 imitation iterations, 60 PPO iterations, seed
5200000, and output `one-deck-candidate`.

Search-guided training (selected run):

```sh
RAYON_NUM_THREADS=3 .venv/bin/python training/train.py --threads 2 train \
  --architecture candidate --width 64 \
  --output training/runs/one-deck-search-distill-seed2 \
  --players 3 4 5 6 --decks 1 --seed 5400000 \
  --validation-seed 6000000 --validation-seeds 200 \
  --games-per-iteration 128 --teacher search-16 \
  --imitation-iterations 20 --dagger-iterations 20 --ppo-iterations 40 \
  --validate-every 10 --suit-augmentation --balanced
```

The independent first training seed used seed 5300000 and output
`one-deck-search-distill`. The second seed's best checkpoint was immediately
after imitation/DAgger; its subsequent PPO did not improve validation.

Diagnostics and validation ablations:

```sh
.venv/bin/python training/diagnose.py \
  --model training/runs/rummy-v1/best.pt \
  --output training/runs/one-deck-diagnostics-stratified.json

.venv/bin/python training/ablate.py \
  --models training/runs/one-deck-search-distill/best.pt \
  --search 0 -1 16 64 --output training/runs/one-deck-distill-ablation.json
```

`-1` means endgame proof search only. Diagnostics stratify disagreement queries
across leads, responses, and two-player endings. Their paired continuation
losses are conditional diagnostics, not estimates of tournament improvement.

## Final evaluation and serving

The final split starts at 7000000, disjoint from training, diagnostics, and
validation. It was reserved before examining final results. Rotate the
candidate through every seat; keep the war strategy fixed.

```sh
.venv/bin/python training/train.py --threads 2 evaluate \
  --model training/runs/one-deck-v2/best.pt --players 3 4 5 6 --decks 1 \
  --seed 7000000 --seeds 2000 --opponents simple --native \
  --search-samples 16 --search-control \
  --output training/runs/one-deck-v2/evaluation-hybrid.json

.venv/bin/python training/train.py --threads 2 evaluate \
  --model training/runs/one-deck-v2/best.pt --players 3 4 5 6 --decks 1 \
  --seed 7000000 --seeds 2000 --opponents simple --native \
  --output training/runs/one-deck-v2/evaluation-policy.json

.venv/bin/python training/train.py --threads 2 evaluate \
  --model training/runs/one-deck-v2/best.pt --players 3 4 --decks 1 \
  --seed 7100000 --seeds 500 --opponents simulate-128 --native \
  --search-samples 16 \
  --output training/runs/one-deck-v2/evaluation-search-opponents.json

GOAT_MODEL=training/runs/one-deck-v2/best.json GOAT_SEARCH_SAMPLES=16 \
  cargo run --release -p goat_server
```

Omit `GOAT_SEARCH_SAMPLES` for the learned policy alone; use `GOAT_ENDGAME=1`
for just the endgame solver. Search samples are per legal move, not a total
rollout budget or a fixed latency bound. `simulate-128` is the older bot with
128 total rollouts, restricted root moves, and full-information continuations;
it is not the actual three-second bot. Performance claims here cover one deck
and the tested opponents/player counts only.
