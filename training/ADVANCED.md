# Selected one-deck rummy bot

The server runs this policy by default through `Selected::default()` with bundled
schema-1 action-scoring weights. Run `cargo run --release -p goat_server`; no
model path, search flags, or Python runtime are needed. Multi-deck games retain
the original server strategy.

The selected implementation keeps public ownership separate from private
knowledge, samples only hidden cards, and gives each simulated player its own
redacted view. It compares every legal move with simple-policy continuations,
using 16 sampled worlds per action (maximum 1,024 rollouts). Repeated worlds are
collapsed when card accounting determines all hands. A cached two-player proof
solver uses a 26-card/200,000-node limit; exhaustion supplies no proof. Unknown
cards are sampled uniformly, without inference from past opponent choices.

The learned scorer supplies move preferences and breaks equal search estimates.
Its weights are unchanged from the selected one-deck-v3 checkpoint. The bundled
asset omits training-only neural layers. The cleanup reproduced all 2,615
selected-policy decisions and outcomes in 12 complete games across 3–6 players.

The discarded portfolio, tree-search, timed/adaptive allocation, weighted
heuristic/evolution, all-action/schema-2 learning, and older duplicate search
implementations have been removed. Their historical run artifacts remain under
`training/runs`. They did not improve the selected aggregate validation result.

## Evaluate

Build the training extension using [README.md](README.md), then:

```sh
RAYON_NUM_THREADS=4 .venv/bin/python training/advanced.py evaluate \
  --model training/runs/one-deck-v3/best.pt --seed 13000000 --seeds 200 \
  --variants prior selected --output training/runs/rummy-current.json
```

This compares the learned prior with selected public-information search. The
historical evaluation below compared the selected policy against the older
search; it is preserved as a record, not relabeled as the new comparison.
The current mixed opponent suite uses the selected public-information search
where the old suite used the older search. See `configs/one-deck-advanced.json`
for the evaluation settings matching the Rust defaults.

## Final held-out evaluation

The locked configuration reduced aggregate goat rate from **8.49% to 7.54%** across 21,600 fresh candidate games: **11.2% relative reduction**.
The paired 95% bootstrap interval for the change is [-1.48, -0.44] percentage points; the interval for relative reduction is [5.3%, 16.8%].
Intervals resample complete deal clusters, retaining all seat rotations and configurations together. The selected weights/configuration were not tuned on these results.

| Opponent | Players | Old goat rate | New goat rate | Paired change, pp (95% CI) |
| --- | ---: | ---: | ---: | ---: |
| simple | 3 | 7.83% | 6.83% | -1.00 [-2.38, +0.38] |
| simple | 4 | 2.50% | 1.38% | -1.12 [-2.13, -0.12] |
| simple | 5 | 1.10% | 0.70% | -0.40 [-1.18, +0.38] |
| simple | 6 | 0.67% | 0.83% | +0.17 [-0.53, +0.86] |
| simulate-128 | 3 | 20.17% | 20.33% | +0.17 [-1.83, +2.16] |
| simulate-128 | 4 | 20.00% | 18.25% | -1.75 [-4.75, +1.25] |
| simulate-128 | 5 | 17.10% | 14.90% | -2.20 [-5.14, +0.74] |
| simulate-128 | 6 | 13.50% | 11.33% | -2.17 [-4.63, +0.30] |
| mixed | 3 | 11.83% | 9.67% | -2.17 [-4.02, -0.31] |
| mixed | 4 | 6.25% | 5.62% | -0.62 [-2.03, +0.78] |
| mixed | 5 | 5.50% | 4.80% | -0.70 [-2.31, +0.91] |
| mixed | 6 | 3.42% | 3.42% | -0.00 [-1.23, +1.23] |

Individual cells have wider intervals than the aggregate. These results establish performance only for the tested one-deck configurations and opponent suite.

There were **0 illegal actions, 0 action-limit failures, and 0 truncated selected-policy rollouts** in this final test.

