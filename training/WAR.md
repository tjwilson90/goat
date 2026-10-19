# Selected one-deck war bot

All server bots now use conservative full-game war search by default. Start the
server with `cargo run --release -p goat_server`; the selected rummy scorer and
both search configurations require no environment variables or training files.
Multi-deck war retains the adaptive baseline.

Search compares every legal action using common sampled worlds. Each branch
continues through the rest of war, actual dreck redistribution, and simple rummy.
The objective is avoiding the final goat. Public winnings and the current trick
remain fixed; only private opposing hands, the draw pile, and unrevealed trump
are sampled. Each simulated player receives its own redacted view. The selected
continuations use `AdaptSimple` in war and `rummy_simple` in rummy.

The default compares up to 64 worlds per legal move, capped at 512 total rollouts
and 4,096 steps per rollout. It keeps the adaptive move unless estimated goat
risk improves by more than 0.10. Incomplete rounds are excluded for every action.
War search runs on Rayon rather than blocking the asynchronous I/O thread.
Human action timing can differ from the deterministic round-robin simulator.

The discarded six-card-target, hand-shape, and opponent-portfolio policies and
root-policy alternatives have been removed. The baseline policies predate this
work and remain available for rollout continuations and evaluation. The cleanup
preserved all 2,615 decisions and outcomes in 12 complete selected-policy games.

## Evaluate

```sh
RAYON_NUM_THREADS=4 .venv/bin/python training/war.py \
  --seed 13100000 --seeds 200 --rummy advanced --variants adapt selected \
  --output training/runs/war-current.json
```

The default scorer is bundled; `--model` optionally overrides it for evaluation.
`--rummy simple` instead gives every seat the simple rummy policy. The candidate
rotates through all seats, with paired deal-clustered confidence intervals.
Proof caches reset before each game to remove worker-scheduling dependence.
Settings matching the serving defaults are in `configs/one-deck-war.json` and
`configs/one-deck-advanced.json`.

The results below are the original held-out evaluations, using the locked policy
and 500/200 seeds per player count for simple/advanced rummy. Historical mixed
war tables included the now-removed target heuristic; current mixed tables use
only the four existing adaptive, duck, cover, and top policies. These are different
opponent suites, so new mixed-table results should not be labeled exact reruns.

## Final held-out results

| Rummy policy shared by every player | Games per policy | AdaptSimple goat rate | Selected goat rate | Relative reduction (paired 95% CI) |
| --- | ---: | ---: | ---: | ---: |
| Simple | 18,000 | 21.31% | 16.31% | 23.5% [20.6%, 26.2%] |
| Advanced | 7,200 | 21.85% | 18.74% | 14.2% [9.2%, 19.2%] |

The advanced-rummy per-cell results are below. Negative differences favor the
new war policy; intervals that include zero do not establish an improvement
for that individual setting. The strongest gains are at smaller tables.

| War opponents | Players | AdaptSimple | Selected | Difference, percentage points (95% CI) |
| --- | ---: | ---: | ---: | ---: |
| adapt | 3 | 33.33% | 21.00% | -12.33 [-15.83, -8.83] |
| adapt | 4 | 25.00% | 22.25% | -2.75 [-5.75, +0.38] |
| adapt | 5 | 20.00% | 21.10% | +1.10 [-1.50, +3.70] |
| adapt | 6 | 16.67% | 16.42% | -0.25 [-2.25, +1.83] |
| mixed | 3 | 39.17% | 22.17% | -17.00 [-22.17, -11.83] |
| mixed | 4 | 21.75% | 18.00% | -3.75 [-7.75, +0.12] |
| mixed | 5 | 17.70% | 18.50% | +0.80 [-2.10, +3.80] |
| mixed | 6 | 15.58% | 14.58% | -1.00 [-3.17, +1.17] |

Across advanced-rummy cells, median war decision time ranged from 1.29
to 3.64 ms; the per-cell p95 ranged from 3.30 to 9.37 ms.
The largest observed war decision was 65.10 ms. These are observed latencies,
not hard time limits. The two final comparisons completed without illegal
actions, stalled games, or truncated rollouts.

